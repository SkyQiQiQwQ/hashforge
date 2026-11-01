# Hashforge

Hashforge is a fast, lightweight Rust desktop app for calculating file hashes, comparing a file with original text, and verifying a known digest. The graphical app is the default program; a separate command-line binary is included for scripts and terminal use.

SHA-256 is selected by default. Files are read in 1 MiB chunks, so memory use stays bounded for large inputs. When calculating multiple formats, the app reads the file once.

## 下载与使用（Windows）

- [下载图形界面 EXE](https://github.com/SkyQiQiQwQ/hashforge/releases/latest/download/hashforge.exe)
- [下载命令行 EXE](https://github.com/SkyQiQiQwQ/hashforge/releases/latest/download/hashforge-cli.exe)
- [版本说明与 SHA-256 校验文件](https://github.com/SkyQiQiQwQ/hashforge/releases/latest)

发布版本适用于 Windows x64。下载 `hashforge.exe` 后直接运行，无需安装 Rust，也无需安装程序。

界面采用极简 Material 3 风格，Windows 上优先使用微软雅黑。选择或拖入文件后默认计算 SHA-256，可切换算法并一键复制结果。在下方输入框粘贴预期摘要或输入原文，程序会自动识别并比对，无需点击验证按钮。摘要支持全大写、全小写及混合大小写，原文按实际内容严格校验。

## Linux / macOS 下载

在 [Releases](https://github.com/SkyQiQiQwQ/hashforge/releases/latest) 下载对应平台的压缩包：

| 平台 | 文件 |
| --- | --- |
| Windows x64 | `hashforge-windows-x64.zip` |
| Linux x64 | `hashforge-linux-x64.tar.gz` |
| macOS Apple Silicon | `hashforge-macos-arm64.tar.gz` |
| macOS Intel | `hashforge-macos-x64.tar.gz` |

压缩包包含 GUI、CLI、README 和许可证。Linux / macOS 解压后运行 `./hashforge` 或 `./hashforge-cli`。Linux 二进制在 Ubuntu 22.04 上构建，GUI 需要可用的桌面图形环境。所有下载文件的 SHA-256 值见 Release 附件 `SHA256SUMS.txt`。

## CI 自动发布

推送到 `main` 分支后，CI 会先在 Windows、Linux、macOS 上执行格式检查、测试和严格 Clippy，全部通过后构建四个目标并发布到 Releases。也可在 Actions 的 `CI` 页面选择 `main` 分支后使用 **Run workflow** 手动构建。Pull Request 和其他分支只运行检查，不发布。

Tag 使用北京时间 `build-yyyyMMdd-HHmm`（例如 `build-20261102-0930`）；Git Tag 不允许空格，因此 Release 标题保留为 `build-yyyyMMdd HHmm`。同一分钟的重复发布会等待下一分钟，避免覆盖已有版本。所有平台构建完成后才会公开 Release。

## Desktop app

Build the Windows executable with:

```powershell
cargo build --release --bin hashforge
```

Run `target/release/hashforge.exe`. On Windows, the release executable is standalone and does not open a console window.

The app calculates a file digest as soon as a file is selected or dropped. The single input below accepts original UTF-8 text or a full-length hexadecimal digest; Hashforge automatically detects the input type and compares it with the file after typing pauses. Results can be copied from the digest row.

Digest verification accepts uppercase, lowercase, or mixed-case hexadecimal letters. The desktop app ignores surrounding whitespace when recognizing a digest. Original text is compared using its exact UTF-8 bytes, including case and whitespace.

## Command line

```powershell
cargo run --release --bin hashforge-cli -- hash .\archive.zip
cargo run --release --bin hashforge-cli -- hash .\archive.zip --algorithm sha256,sha3-256,blake3
cargo run --release --bin hashforge-cli -- hash .\archive.zip --all
cargo run --release --bin hashforge-cli -- text "original text"
cargo run --release --bin hashforge-cli -- compare .\message.txt --text "original text"
cargo run --release --bin hashforge-cli -- verify .\archive.zip --hash <expected-hex-digest>
```

The CLI executable is `target/release/hashforge-cli.exe` on Windows. `compare` hashes the UTF-8 bytes of the supplied text and the file bytes. A match returns exit code 0, a mismatch returns 1, and invalid arguments or I/O errors return 2. Use `-` as the hash input file to read raw bytes from standard input.

## Algorithms

MD5, SHA-1, SHA-224/256/384/512, SHA3-224/256/384/512, BLAKE2b, BLAKE2s, BLAKE3, CRC32, and XXH3-64.

MD5, SHA-1, CRC32, and XXH3 are included for compatibility or accidental-corruption detection; they are not suitable for security-sensitive integrity checks. XXH3 is available when non-cryptographic throughput is the priority.

## Development

Install the stable Rust toolchain, then run:

```text
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked --bin hashforge
```

## License

MIT. See [LICENSE](LICENSE).
