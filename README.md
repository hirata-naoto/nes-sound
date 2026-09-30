# nes-sound

RP2350（Raspberry Pi Pico 2）で動作する、Embassy-rs ベースの NES APU サウンドサンプルです。2つのパルス波、三角波、ノイズ、DPCM を合成し、PWM 音声を GPIO1 から出力します。

## 構成

- `src/apu.rs` - ホストでもテストできる APU チャンネル、ミキサ、DPCM デコーダ
- `src/main.rs` - Embassy の非同期タイマーと PWM を使った Pico 2 ファームウェア、デモ曲

## 配線

- GPIO1 を RC ローパスフィルタ経由でアンプまたはアクティブスピーカーへ接続します。
- 例: `GPIO1 -> 330Ω -> 出力`、出力点から `0.01uF` を GND へ接続します。

PWM をスピーカーへ直接接続せず、アンプまたは十分なフィルタを使用してください。

## ビルドと書き込み

Rust の thumbv8m ターゲットと `elf2uf2-rs` をインストールします。

```bash
rustup target add thumbv8m.main-none-eabihf
cargo install elf2uf2-rs
cargo build --release --features firmware
elf2uf2-rs target/thumbv8m.main-none-eabihf/release/nes-sound nes-sound.uf2
```

Pico 2 を BOOTSEL モードで接続し、生成された `nes-sound.uf2` をドライブへコピーします。GPIO1 の PWM キャリアを平均 22.05 kHz のサンプル更新で変調し、起動後すぐに DPCM を含むデモループを再生します。

## ホストテスト

```bash
cargo test --target x86_64-unknown-linux-gnu
```

APU は `no_std` の Rust ライブラリとして分離されており、マイクロコントローラー向けターゲットなしでテストできます。

## DPCM

`Apu::start_dpcm` は静的メモリ上のサンプル、NES の 16 段階ビットレート、ループ再生を受け付けます。DPCM は LSB-first でデルタカウンタを更新し、NES の TND 非線形ミキサへ入力されます。`Apu::stop_dpcm` はサンプル読み出しを停止し、現在の DAC レベルを保持します。
