# tsgo: Native TypeScript Compiler Integration for Zed

This extension integrates the native, Go-based TypeScript compiler and language server into the Zed editor, delivering enhanced performance and efficiency for TypeScript development.

## 🚀 Why the native compiler?

TypeScript 7.0 ports the compiler from its JavaScript implementation to a native version written in Go, delivering significant performance improvements:

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

_Note_: While TypeScript 7.0 is now stable, some tools (e.g. `typescript-eslint`) still require the TypeScript 6.0 API, since a new native compiler API isn't expected until TypeScript 7.1. This extension only integrates the compiler and language server, so it isn't affected by that limitation.

### Basic Setup

Enable `tsgo` in your Zed settings:

```jsonc
{
  "languages": {
    "TypeScript": {
      "language_servers": ["tsgo", "!vtsls", "!typescript-language-server", "..."],
    },
    "TSX": {
      "language_servers": ["tsgo", "!vtsls", "!typescript-language-server", "..."],
    },
  },
}
```

You can also use `tsgo` in tandem with other language servers (e.g. `typescript-language-server` or `vtsls`). Zed will use `tsgo` for features it supports and fallback to the next language server in the list for unsupported features.
To do that with `vtsls`, use:

```jsonc
{
  "languages": {
    "TypeScript": {
      "language_servers": ["tsgo", "vtsls", "!typescript-language-server", "..."],
    },
    "TSX": {
      "language_servers": ["tsgo", "vtsls", "!typescript-language-server", "..."],
    },
  },
}
```

### Advanced Configuration

#### Specifying a Package Version

By default, the extension installs and uses the latest version of the [`typescript`](https://www.npmjs.com/package/typescript?activeTab=versions) npm package. To pin a specific version:

```json
{
  "lsp": {
    "tsgo": {
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

You can also pin to a nightly build by using a version published under the `next` dist-tag, e.g. `"package_version": "7.1.0-dev.20260710.1"` (nightly builds are published to the stand

## 🧪 Status

This extension is in early development stages. While it offers significant performance benefits, some features may be incomplete or unstable. Contributions and feedback are welcome to improve its functionality.
