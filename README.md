# tsgo: Native TypeScript Compiler Integration for Zed

This extension integrates `tsgo`, Microsoft's native Go-based TypeScript compiler, into the Zed editor, delivering enhanced performance and efficiency for TypeScript development.

## 🚀 Why `tsgo`?

Microsoft is transitioning the TypeScript compiler from its JavaScript implementation to a native version written in Go, aiming for significant performance improvements:

- **Faster Compilation**: Achieves up to 10x speed improvements in large projects.
- **Reduced Memory Usage**: Optimized memory handling in native execution.
- **Improved Editor Performance**: Faster IntelliSense and language services.
- **Scalability**: Better handling of large codebases.

> _Example Benchmarks_:
>
> - **VS Code**: 77.8s → 7.5s (10.4x speedup)
> - **Playwright**: 11.1s → 1.1s (10.1x speedup)
> - **TypeORM**: 17.5s → 1.3s (13.5x speedup)
>
> _Source: [Microsoft Developer Blog](https://devblogs.microsoft.com/typescript/typescript-native-port/)_

## 🛠 Installation

1. Open Zed's Extensions page.
2. Search for `tsgo` and install the extension.

## ⚙️ Configuration

_Note_: `tsgo` is currently in preview and may not support all features of the standard `tsc` compiler.

### Basic Setup

Enable `tsgo` in your Zed settings:

```json
{
  "languages": {
    "TypeScript": {
      "language_servers": ["tsgo"]
    }
  }
}
```

You can also use `tsgo` in tandem with other language servers (e.g. `typescript-language-server` or `vtsls`). Zed will use `tsgo` for features it supports and fallback to the next language server in the list for unsupported features.
To do that with `vtsls`, use:

```json
{
  "languages": {
    "TypeScript": {
      "language_servers": ["tsgo", "vtsls"]
    }
  }
}
```

### Advanced Configuration

#### Specifying a Package Version

By default, the extension installs and uses the latest version of the `@typescript/native-preview` [npm package](https://www.npmjs.com/package/@typescript/native-preview?activeTab=versions). To pin a specific version:

```json
{
  "lsp": {
    "tsgo": {
      "settings": {
        "package_version": "7.0.0-dev.20251029.1"
      }
    }
  }
}
```

This is useful for:

- Ensuring consistent behavior across the project
- Testing specific versions
- Avoiding automatic updates that might introduce issues

#### Custom Binary Path

By default, the extension automatically installs and manages the `tsgo` binary. If you prefer to use a custom binary (e.g., a locally built version), you can specify the folder containing the binary in your settings:

```json
{
  "lsp": {
    "tsgo": {
      "settings": {
        "tsdk": "/path/to/folder/containing/tsgo"
      }
    }
  }
}
```

The `tsdk` setting should point to the folder containing the `tsgo` binary (not the binary itself) and can be either:
- An absolute path (e.g., `/usr/local/bin`)
- A relative path from your workspace root (e.g., `./bin`)

When a custom `tsdk` path is configured, the extension will skip automatic installation and use the binary from your specified folder instead.

## 🧪 Status

This extension is in early development stages. While it offers significant performance benefits, some features may be incomplete or unstable. Contributions and feedback are welcome to improve its functionality.
