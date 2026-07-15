# tsgo: Native TypeScript Compiler Integration for Zed

This extension integrates TypeScript v7's native Go-based compiler and language server into the Zed editor, delivering enhanced performance and efficiency for TypeScript development.

## 🚀 Why the native compiler?

With TypeScript 7, Microsoft shipped the TypeScript compiler as a native version written in Go, with significant performance improvements:

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
2. Search for `TypeScript Language Server` and install the extension.

## ⚙️ Configuration

### Basic Setup

Enable `typescript-ls` in your Zed settings:

```jsonc
{
    "languages": {
        "TypeScript": {
            "language_servers": [
                "typescript-ls",
                "!vtsls",
                "!typescript-language-server",
                "...",
            ],
        },
        "TSX": {
            "language_servers": [
                "typescript-ls",
                "!vtsls",
                "!typescript-language-server",
                "...",
            ],
        },
    },
}
```

You can also use `typescript-ls` in tandem with other language servers (e.g. `typescript-language-server` or `vtsls`). Zed will use `typescript-ls` for features it supports and fallback to the next language server in the list for unsupported features.
To do that with `vtsls`, use:

```jsonc
{
    "languages": {
        "TypeScript": {
            "language_servers": [
                "typescript-ls",
                "vtsls",
                "!typescript-language-server",
                "...",
            ],
        },
        "TSX": {
            "language_servers": [
                "typescript-ls",
                "vtsls",
                "!typescript-language-server",
                "...",
            ],
        },
    },
}
```

### Advanced Configuration

#### Specifying a Package Version

By default, the extension installs and uses the latest version of the `typescript` [npm package](https://www.npmjs.com/package/typescript?activeTab=versions). To pin a specific version (must be >= 7.0.0, older versions have no native language server):

```json
{
    "lsp": {
        "typescript-ls": {
            "settings": {
                "package_version": "7.0.2"
            }
        }
    }
}
```

This is useful for:

- Ensuring consistent behavior across the project
- Testing specific versions
- Avoiding automatic updates that might introduce issues
