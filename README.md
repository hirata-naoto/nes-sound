# nes-sound

RP2350（Pico 2）でファミコン APU の DPCM 以外をできるだけ忠実に鳴らす最小構成のサンプルです。
2つのパルス波、1つの三角波、1つのノイズを NES 互換のシーケンサとミキサで合成し、PWM 出力でスピーカーやアンプへ送ります。

## 構成

- `/src/nes_apu.c` - ファミコン APU 風の簡易音源
- `/src/main.c` - RP2350 の PWM 出力とデモ曲再生
- `/tests/nes_apu_smoke_test.c` - ホストで実行できる簡易スモークテスト

## 配線

- `GPIO1` を RC ローパスフィルタ経由でアンプまたはアクティブスピーカーへ接続
- 例: `GPIO1 -> 330Ω -> 出力`, 出力点から `0.01uF` を GND へ

PWM を直接スピーカーへつなぐのではなく、必ずアンプか十分なフィルタを挟んでください。

## ビルド

Pico SDK を用意して `PICO_SDK_PATH` を設定してください。

```bash
export PICO_SDK_PATH=/path/to/pico-sdk
cmake -S . -B build -DPICO_BOARD=pico2
cmake --build build
```

生成される `build/nes_sound.uf2` を Pico 2 に書き込むと、起動後すぐにファミコン風デモループを再生します。

## ホストでの簡易確認

クロスコンパイル環境がなくても、音源ロジックだけは GCC で確認できます。

```bash
gcc -std=c11 -Wall -Wextra -Werror -I./src tests/nes_apu_smoke_test.c src/nes_apu.c -lm -o /tmp/nes_apu_smoke_test
/tmp/nes_apu_smoke_test
```

## メモ

- DPCM チャンネルは未実装です
- パルス 4 種デューティ、32 ステップ三角波、15-bit LFSR ノイズ、非線形ミキサを NES に合わせて再現しています
- ノイズは通常モードと短周期モードを切り替えられます
- `k_demo_song` を差し替えることで任意のメロディへ変更できます