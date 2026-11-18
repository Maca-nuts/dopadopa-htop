# dopadopa-htop

A colorful, decorated terminal system monitor inspired by `htop`. It displays CPU and memory usage, history sparklines, a sortable-by-CPU process list, and NVIDIA GPU core/VRAM telemetry when `nvidia-smi` is available.

## Run

Install Rust, then:

```sh
cargo run --release
```

Keys: `q`/`Esc` quit, `r` refresh immediately, and `↑`/`↓` (or `j`/`k`) select a process.

GPU data comes from:

```sh
nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits
```

If NVIDIA drivers or `nvidia-smi` are unavailable, the rest of the monitor continues to work and the GPU panel reports that telemetry is unavailable.
