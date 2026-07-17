# tsgo: TypeScript 7 language server for Zed

This extension runs TypeScript 7's native, Go-based language server in Zed for JavaScript, JSX,
TypeScript, and TSX.

## Installation

Open Zed's Extensions page, search for `TypeScript Language Server`, and install the extension.

The server id is `typescript-ls`. To make it the primary TypeScript server:

```jsonc
{
  "languages": {
    "JavaScript": {
      "language_servers": ["typescript-ls", "!vtsls", "!typescript-language-server", "..."]
    },
    "JSX": {
      "language_servers": ["typescript-ls", "!vtsls", "!typescript-language-server", "..."]
    },
    "TypeScript": {
      "language_servers": ["typescript-ls", "!vtsls", "!typescript-language-server", "..."]
    },
    "TSX": {
      "language_servers": ["typescript-ls", "!vtsls", "!typescript-language-server", "..."]
    }
  }
}
```

You can also keep another server later in the list for features the native server does not yet
support:

```jsonc
{
  "languages": {
    "TypeScript": {
      "language_servers": ["typescript-ls", "vtsls", "!typescript-language-server", "..."]
    },
    "TSX": {
      "language_servers": ["typescript-ls", "vtsls", "!typescript-language-server", "..."]
    }
  }
}
```

## How the server runs

TypeScript 7's language server is a native executable in the platform-specific
`@typescript/typescript-<platform>-<arch>` npm packages, launched in LSP mode:

```sh
tsc --lsp --stdio
```

The extension executes that native binary directly when it can locate it next to the resolved
`typescript` package. Otherwise it launches the package's `bin/tsc` Node shim, which uses
Microsoft's own package resolution and covers pnpm and unusual install layouts. The fallback uses
the worktree's `node` (including Volta/fnm shims) when available, or Zed's bundled Node.

## TypeScript package resolution

The extension resolves the TypeScript 7+ package in this order:

1. A TypeScript dependency in the worktree root's `package.json`. The extension checks
   `dependencies`, `devDependencies`, and `peerDependencies`, including `npm:` aliases under any
   dependency name. Declarations that clearly select TypeScript 6 are skipped; when installed
   package metadata is visible through Zed's worktree API, its version is checked as well.
2. A managed `typescript` install in the extension's working directory.

This supports Microsoft's TypeScript 6/7 side-by-side setup, for example:

```json
{
  "devDependencies": {
    "@typescript/native": "npm:typescript@^7",
    "typescript": "npm:@typescript/typescript6@^6"
  }
}
```

The TypeScript 7 alias is selected and the TypeScript 6 compatibility package is ignored.

TypeScript 6 and older cannot run the native LSP and are rejected. Use Zed's built-in TypeScript
support for those versions.

## Development

Install Rust with `rustup`, then run `zed: install dev extension` and select this directory.

After source changes, rebuild the extension from the Extensions page. Run Zed with
`zed --foreground` or use `zed: open log` to inspect extension and language-server errors.

<!-- markdownlint-disable-file MD013 -->
