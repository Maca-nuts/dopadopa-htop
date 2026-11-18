# dopadopa-htop

`htop` に着想を得た、カラフルなターミナル用システムモニターです。CPU とメモリの使用率、対応センサーから取得できる CPU 温度、使用率の履歴、CPU 使用率順のプロセス一覧を表示します。`nvidia-smi` が利用できる場合は、NVIDIA GPU の使用率、VRAM 使用量、温度も表示します。

## インストールと実行

Rust をインストールした後、次のコマンドで `dtop` をインストールします。

```sh
cargo install --path .
```

ターミナルで次のコマンドを実行すると起動します。

```sh
dtop
```

開発時は、インストールせずに `cargo run --release --bin dtop` で実行できます。

## 操作方法

- `q` / `Esc`：終了
- `r`：すぐに更新
- `↑` / `↓` または `j` / `k`：プロセスを選択

## GPU 情報

GPU の情報は、次のコマンドで取得します。

```sh
nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv,noheader,nounits
```

NVIDIA ドライバーまたは `nvidia-smi` を利用できない場合も、それ以外の情報は表示されます。GPU パネルには情報を取得できない旨が表示されます。
