#!/usr/bin/env node
// Markdownの段落が途中で改行されていないことを確認する。
//
// 改行は段落の終わりにだけ置く。表示幅に合わせた折り返しは表示側が行うものであり、
// 文の途中に文字数で改行を入れない。英語も日本語も同じである。
//
// 引数なしで実行すると、repositoryが追跡する`.md`と未追跡の`.md`をすべて確認する。
// fileを引数に渡せばそのfileだけを確認する。`--fix`を付けると、段落の続きの行を前の行へ
// 結合して書き戻す。code block、front matter、表、HTML、見出し、引用、水平線は触らない。
//
// 結合するときの区切りは、境目のどちらかがCJKの文字なら空白なし、どちらもASCIIなら
// 空白1つとする。日本語の文中に英単語が2語並ぶ箇所(例: `unit test`)は後者に当たる。

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const FENCE = /^(\s*)(`{3,}|~{3,})(.*)$/;
const HEADING = /^\s{0,3}#{1,6}(\s|$)/;
const HORIZONTAL_RULE = /^\s{0,3}([-*_])(\s*\1){2,}\s*$/;
const TABLE_ROW = /^\s*\|/;
const HTML_BLOCK = /^\s*<[a-zA-Z/!]/;
const BLOCKQUOTE = /^\s*>/;
const LIST_ITEM = /^\s*([-*+]|\d+[.)])\s+/;
const INDENTED_CODE = /^ {4,}\S/;

const CJK_RANGES: ReadonlyArray<readonly [number, number]> = [
  [0x3000, 0x303f], // CJKの記号と句読点
  [0x3040, 0x309f], // ひらがな
  [0x30a0, 0x30ff], // カタカナ
  [0x31f0, 0x31ff], // カタカナ拡張
  [0x3300, 0x33ff], // CJK互換
  [0x3400, 0x4dbf], // CJK統合漢字拡張A
  [0x4e00, 0x9fff], // CJK統合漢字
  [0xf900, 0xfaff], // CJK互換漢字
  [0xff00, 0xffef], // 全角形
];

type Paragraph = Array<readonly [lineNumber: number, line: string]>;

function main(): number {
  const args = process.argv.slice(2);
  const fix = args.includes("--fix");
  const given = args.filter((arg) => arg !== "--fix");
  const paths = given.length > 0 ? given : listMarkdownFiles();

  const failures: string[] = [];
  for (const path of paths) {
    const text = readFileSync(path, "utf8");
    const { joined, continuationLines } = unwrap(text);
    if (joined === text) {
      continue;
    }
    if (fix) {
      writeFileSync(path, joined);
      console.log(`${path}: ${continuationLines.length}行を前の行へ結合した`);
    } else {
      failures.push(...continuationLines.map((line) => `${path}:${line}`));
    }
  }

  if (failures.length === 0) {
    return 0;
  }
  for (const failure of failures) {
    console.error(`${failure}: 段落の途中で改行されている`);
  }
  console.error(
    `\n${failures.length}箇所。改行は段落の終わりにだけ置き、文の途中では改行しない。` +
      "`mise run fmt`か`node scripts/check_md_line_breaks.mts --fix`で結合できる。",
  );
  return 1;
}

function listMarkdownFiles(): string[] {
  const output = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "*.md"],
    { encoding: "utf8" },
  );
  return [...new Set(output.split("\0").filter((path) => path !== ""))].sort();
}

/** 段落の続きの行を前の行へ結合した本文と、結合した行の行番号を返す。 */
function unwrap(text: string): { joined: string; continuationLines: number[] } {
  const lines = text.split("\n");
  const out: string[] = [];
  const continuationLines: number[] = [];
  let paragraph: Paragraph = []; // 空なら段落の外にいる。

  const flush = (): void => {
    if (paragraph.length === 0) {
      return;
    }
    let joined = paragraph[0][1].trimEnd();
    for (const [lineNumber, line] of paragraph.slice(1)) {
      const rest = line.trim();
      if (rest !== "") {
        continuationLines.push(lineNumber);
        joined += separator(joined, rest) + rest;
      }
    }
    out.push(joined);
    paragraph = [];
  };

  let index = 0;
  if (lines.length > 0 && lines[0].trim() === "---") {
    // front matter。閉じる`---`まで触らない。
    out.push(lines[0]);
    index = 1;
    while (index < lines.length) {
      out.push(lines[index]);
      index += 1;
      if (lines[index - 1].trim() === "---") {
        break;
      }
    }
  }

  let fence: readonly [char: string, length: number] | null = null; // code blockの中にいる間だけ持つ。
  let inHtml = false;
  while (index < lines.length) {
    const line = lines[index];
    const lineNumber = index + 1;
    index += 1;
    const stripped = line.trim();

    if (fence !== null) {
      out.push(line);
      const closing = FENCE.exec(line);
      if (
        closing !== null &&
        closing[2][0] === fence[0] &&
        closing[2].length >= fence[1] &&
        closing[3].trim() === ""
      ) {
        fence = null;
      }
      continue;
    }
    const opening = FENCE.exec(line);
    if (opening !== null) {
      flush();
      fence = [opening[2][0], opening[2].length];
      out.push(line);
      continue;
    }
    if (stripped === "") {
      flush();
      inHtml = false;
      out.push(line);
      continue;
    }
    if (inHtml) {
      out.push(line);
      continue;
    }
    if (HTML_BLOCK.test(line)) {
      flush();
      inHtml = true;
      out.push(line);
      continue;
    }
    if (
      TABLE_ROW.test(line) ||
      HEADING.test(line) ||
      HORIZONTAL_RULE.test(line) ||
      BLOCKQUOTE.test(line)
    ) {
      flush();
      out.push(line);
      continue;
    }
    if (LIST_ITEM.test(line)) {
      flush();
      paragraph.push([lineNumber, line]);
      continue;
    }
    if (paragraph.length === 0 && INDENTED_CODE.test(line)) {
      out.push(line);
      continue;
    }
    paragraph.push([lineNumber, line]);
  }
  flush();
  return { joined: out.join("\n"), continuationLines };
}

function separator(before: string, after: string): string {
  if (isCjk(before[before.length - 1]) || isCjk(after[0])) {
    return "";
  }
  return " ";
}

function isCjk(char: string): boolean {
  const code = char.codePointAt(0) ?? 0;
  return CJK_RANGES.some(([low, high]) => low <= code && code <= high);
}

process.exitCode = main();
