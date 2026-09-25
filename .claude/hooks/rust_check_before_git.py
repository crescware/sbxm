#!/usr/bin/env python3
# Claude CodeのPreToolUse hook。Bash toolが`git commit`か`git merge`を実行する前に
# `cargo fmt --check`を、それが通ったら`cargo clippy --all-targets -- -D warnings`を実行する。
# どちらかが失敗したらexit 2でtool呼び出しを拒否し、失敗の出力をClaudeへ返す。
# 両方通ったときはexit 0で、判断を通常の権限確認へ委ねる。
#
# 拒否しそこねるより余分に検査するほうを選ぶ。commandを読み切れないときは、
# `git`と`commit`/`merge`の語が並ぶだけで対象にする。

import json
import os
import re
import shlex
import subprocess
import sys
import time

CHECKS = (
    ["cargo", "fmt", "--check"],
    ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
)
# Claude Codeは時間切れのhookを打ち切り、tool呼び出しを通してしまう。settings.jsonで
# 与えた900秒より手前で検査を打ち切り、拒否として扱う。
DEADLINE_SECONDS = 840
OUTPUT_LIMIT = 10_000
ANSI_ESCAPE = re.compile(r"\x1b\[[0-9;]*m")

SEPARATOR_CHARS = frozenset(";&|()\n`")
REDIRECTION = re.compile(r"[<>&|]*[<>][<>&|]*")
ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=")
NUMBER = re.compile(r"\d+(\.\d+)?[smhd]?")
# 後ろの語をcommandとして実行させる語。
PREFIX_WORDS = frozenset(
    ["!", "{", "if", "then", "elif", "else", "while", "until", "do"]
    + ["time", "command", "builtin", "exec", "nohup", "env", "nice", "sudo", "timeout"]
)
SHELLS = frozenset(["bash", "sh", "zsh", "dash"])
SHELL_SCRIPT_OPTION = re.compile(r"-[A-Za-z]*c[A-Za-z]*")
GIT_OPTIONS_WITH_VALUE = frozenset(
    ["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--config-env", "--super-prefix"]
)
# 中止するだけでmerge commitを作らない。衝突中は検査が通らないため、拒否すると抜け出せない。
MERGE_WITHOUT_COMMIT = frozenset(["--abort", "--quit"])
# heredocの本文はcommandではない。commit messageの引用符が字句解析を崩さないよう取り除く。
HEREDOC = re.compile(
    r"(<<-?[ \t]*(['\"]?)([A-Za-z_]\w*)\2[^\n]*\n)(?:.*?\n)?[ \t]*\3[ \t]*$",
    re.S | re.M,
)
LOOSE_GIT = re.compile(r"\bgit\b.*\b(?:commit|merge)\b", re.S)


def main():
    try:
        payload = json.load(sys.stdin)
        if payload.get("tool_name") != "Bash":
            return 0
        command = payload["tool_input"]["command"]
        cwd = payload.get("cwd") or os.getcwd()
        work_dirs = list(dict.fromkeys(find_work_dirs(command, cwd)))
    except Exception as error:
        return deny(f"hookがtool入力を読めませんでした: {error!r}")

    deadline = time.monotonic() + DEADLINE_SECONDS
    for work_dir in work_dirs:
        for check in CHECKS:
            failure = run_check(check, work_dir, deadline)
            if failure:
                return deny(failure)
    return 0


def find_work_dirs(command, cwd):
    """commandの中で`git commit`/`git merge`が走る作業directoryを、現れる順に返す。"""
    command = HEREDOC.sub(r"\1", command.replace("\\\n", " "))
    try:
        tokens = tokenize(command)
    except ValueError:
        return [cwd] if LOOSE_GIT.search(command) else []

    work_dirs = []
    directory = cwd
    for words in split_commands(tokens):
        words = strip_prefix(words)
        if not words:
            continue
        name, args = os.path.basename(words[0]), words[1:]
        if name in ("cd", "pushd"):
            operands = [arg for arg in args if not arg.startswith("-")]
            directory = resolve(directory, operands[0] if operands else "~")
        elif name in SHELLS:
            script = shell_script(args)
            if script is not None:
                work_dirs += find_work_dirs(script, directory)
        elif name == "git":
            work_dir = git_work_dir(args, directory)
            if work_dir is not None:
                work_dirs.append(work_dir)
    return work_dirs


def tokenize(command):
    lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|()<>\n`")
    lexer.whitespace = " \t\r"
    lexer.whitespace_split = True
    lexer.commenters = ""
    return list(lexer)


def split_commands(tokens):
    words = []
    for token in tokens:
        if token and set(token) <= SEPARATOR_CHARS:
            yield words
            words = []
        else:
            words.append(token)
    yield words


def strip_prefix(words):
    """先頭の代入、redirect、`env`や`if`などを除き、実行されるcommandから始まる語の列を返す。"""
    i = 0
    wrapped = False
    while i < len(words):
        word = words[i]
        if REDIRECTION.fullmatch(word):
            i += 2
        elif ASSIGNMENT.match(word):
            i += 1
        elif word in PREFIX_WORDS:
            wrapped = True
            i += 1
        elif wrapped and (word.startswith("-") or NUMBER.fullmatch(word)):
            i += 1
        else:
            break
    return words[i:]


def shell_script(args):
    """`bash -c 'script'`のscript部分を返す。"""
    for i, arg in enumerate(args[:-1]):
        if SHELL_SCRIPT_OPTION.fullmatch(arg):
            return args[i + 1]
    return None


def git_work_dir(args, directory):
    """`git`の引数がcommitかmerge commitを作るなら、その作業directoryを返す。"""
    i = 0
    while i < len(args) and args[i].startswith("-"):
        if args[i] == "-C" and i + 1 < len(args):
            directory = resolve(directory, args[i + 1])
        i += 2 if args[i] in GIT_OPTIONS_WITH_VALUE else 1
    if i >= len(args):
        return None
    subcommand, rest = args[i], args[i + 1 :]
    if subcommand == "commit":
        return directory
    if subcommand == "merge" and not MERGE_WITHOUT_COMMIT.intersection(rest):
        return directory
    return None


def resolve(directory, path):
    path = os.path.expanduser(os.path.expandvars(path))
    return os.path.normpath(os.path.join(directory, path))


def run_check(check, work_dir, deadline):
    """検査が通ればNoneを、通らなければ拒否の理由を返す。"""
    label = " ".join(check)
    try:
        result = subprocess.run(
            check,
            cwd=work_dir,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            timeout=max(1.0, deadline - time.monotonic()),
        )
    except subprocess.TimeoutExpired:
        return f"`{label}`が{DEADLINE_SECONDS}秒以内に終わりませんでした({work_dir})。"
    except OSError as error:
        return f"`{label}`を{work_dir}で実行できませんでした: {error}"
    if result.returncode == 0:
        return None
    # rustfmtは端末でなくても差分に色を付ける。Claudeへ返す文面から色の指定を除く。
    output = ANSI_ESCAPE.sub("", result.stdout)
    if len(output) > OUTPUT_LIMIT:
        output = output[:OUTPUT_LIMIT] + "\n…(以下省略)"
    return f"`{label}`が失敗しました(exit {result.returncode}, {work_dir})。\n{output}"


def deny(reason):
    print(
        f"{reason}\n"
        "`cargo fmt --check`と`cargo clippy --all-targets -- -D warnings`が両方通るまで、"
        "`git commit`/`git merge`は実行できません。",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main())
