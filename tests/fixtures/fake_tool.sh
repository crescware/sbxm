#!/bin/sh
# testが置くhost toolの代役。testはこのfileへのsymlinkを道具の名前で置き、振る舞いは
# 同じ名前に`.sh`を付けた、実行bitを持たないfileへ書く。ここではそれを読み込むだけであり、
# `$0`と引数はそのまま台本へ渡る。実行可能fileを実行時に書かないため、execされるのは
# repositoryにあるこのfileだけである。
. "$0.sh"
