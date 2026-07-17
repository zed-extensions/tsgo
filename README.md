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

1. `tsdk.path`, when set. It accepts a package root, the package's `lib` directory (the VS Code
   `typescript.tsdk` convention), a `bin/tsc` path, or a platform package containing the native
   executable. The version is checked when Zed exposes the package metadata to the extension;
   otherwise the package's own launcher validates it at startup.
2. A TypeScript dependency in the worktree root's `package.json`. The extension checks
   `dependencies`, `devDependencies`, and `peerDependencies`, including `npm:` aliases under any
   dependency name. Declarations that clearly select TypeScript 6 are skipped; when installed
   package metadata is visible through Zed's worktree API, its version is checked as well.
3. A managed `typescript` install in the extension's working directory.

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

## Managed install version

Managed installs use the latest stable TypeScript release by default. These settings live under
`lsp.typescript-ls.settings`:

| Setting | Install behavior |
| --- | --- |
| `package_version` | Existing setting for any npm version spec, such as `7.0.2`, `next`, or `^7`. |
| `version` | Alias for `package_version`; `package_version` wins when both are present. |
| `updateChannel: "latest"` | Install the latest stable `typescript` package. |
| `updateChannel: "next"` | Install `typescript@next`, TypeScript's nightly channel. |

An explicit `package_version` or `version` wins over `updateChannel`. If the npm registry is
temporarily unavailable, the extension reuses an already-installed managed TypeScript 7+ package.

```json
{
  "lsp": {
    "typescript-ls": {
      "settings": {
        "updateChannel": "next"
      }
    }
  }
}
```

## Runtime settings

The extension owns a small set of launch and installation keys. Everything else in `settings` is
forwarded to the language server through `workspace/configuration`.

| Setting | Meaning |
| --- | --- |
| `package_version` / `version` | Managed npm version spec. |
| `updateChannel` | Managed release channel: `"latest"` or `"next"`. |
| `tsdk.path` | Explicit TypeScript package location. |
| `server.pprofDir` | Adds `--pprofDir <path>` to the server command. |
| `server.goMemLimit` | Sets `GOMEMLIMIT`; accepts integer bytes with an optional `B`, `KiB`, `MiB`, `GiB`, or `TiB` suffix, or `off`. |
| `server.args` | Extra arguments appended after `--lsp --stdio`. |
| `server.env` | Extra environment variables for the server process. |

Dotted and nested forms are both accepted; the nested form wins when both are present.

```jsonc
{
  "lsp": {
    "typescript-ls": {
      "settings": {
        "tsdk": {
          "path": "./node_modules/@typescript/native"
        },
        "server": {
          "pprofDir": "./.typescript-pprof",
          "goMemLimit": "2048MiB",
          "args": [],
          "env": {
            "GOGC": "50"
          }
        }
      }
    }
  }
}
```

The environment precedence, from lowest to highest, is the worktree shell environment,
`server.env`, `server.goMemLimit`, then `binary.env`.

The current server flag surface in `--lsp` mode is `--stdio`, `--pipe <name>`,
`--socket <address>`, and `--pprofDir <path>`. `server.args` can forward future flags without an
extension release.

## Custom binary

`lsp.typescript-ls.binary` can override how the server is launched:

```jsonc
{
  "lsp": {
    "typescript-ls": {
      "binary": {
        // A tsc/tsc.js/tsc.exe path is launched as the server.
        // Any other path is treated as a custom Node executable.
        "path": "/path/to/tsc",
        "arguments": ["--lsp", "--stdio"],
        "env": {
          "GOMEMLIMIT": "4GiB"
        }
      }
    }
  }
}
```

Explicit `arguments` replace all extension-generated arguments. When `arguments` are omitted, a
`tsc`, `tsc.js`, or `tsc.exe` path receives the normal server flags. Any other configured path is
treated as Node and receives the resolved TypeScript `bin/tsc` launcher plus those flags.

## Language server configuration

The server reads the configuration sections `js/ts`, `typescript`, `javascript`, and `editor`.
Place those sections directly in `lsp.typescript-ls.settings`; they are forwarded to the server:

```jsonc
{
  "lsp": {
    "typescript-ls": {
      "settings": {
        "typescript": {
          "preferences": {
            "quoteStyle": "single",
            "importModuleSpecifier": "non-relative",
            "preferTypeOnlyAutoImports": true
          }
        },
        "js/ts": {
          "implicitProjectConfig": {
            "checkJs": true
          }
        }
      }
    }
  }
}
```

Matching Zed's built-in TypeScript support, the extension enables every inlay hint kind and the
references and implementations code lenses server-side by default. Zed's own `inlay_hints` and
`code_lens` settings still control whether they are displayed. User configuration deep-merges over
these defaults and wins at leaf level.

VS Code-style dotted keys are expanded, so settings copied from TypeScript documentation work
without restructuring:

```jsonc
{
  "lsp": {
    "typescript-ls": {
      "settings": {
        "typescript.inlayHints.parameterNames.enabled": "literals",
        "typescript.preferences.quoteStyle": "single",
        "js/ts.implicitProjectConfig.strictNullChecks": true
      }
    }
  }
}
```

When nested and dotted settings conflict, the nested value wins.

Option groups currently read by the server include `inlayHints.*`, `preferences.*`, `suggest.*`,
`format.*`, `referencesCodeLens.*`, `implementationsCodeLens.*`, `validate.*`,
`workspaceSymbols.*`, `autoClosingTags`, and `implicitProjectConfig.*` under `js/ts`. See the
server's authoritative
[`UserPreferences`](https://github.com/microsoft/typescript-go/blob/main/internal/ls/lsutil/userpreferences.go)
definition for the current surface.

## Initialization options

`initialization_options` are forwarded verbatim at server initialization:

```jsonc
{
  "lsp": {
    "typescript-ls": {
      "initialization_options": {
        "enableTelemetry": false,
        "logVerbosity": 2
      }
    }
  }
}
```

Known upstream options include `disablePushDiagnostics`, `codeLensShowLocationsCommandName`,
`userPreferences`, `enableTelemetry`, and `logVerbosity`. Configuration sections can update live;
initialization options only apply at server startup.

## Legacy settings

The extension previously used the server id `tsgo`. For compatibility, `binary`, `settings`, and
`initialization_options` each fall back to `lsp.tsgo` when that field is absent under
`lsp.typescript-ls`.

New configuration should use `lsp.typescript-ls`.

## Known limits

- TypeScript 7 does not yet expose the stable programmatic API needed by embedded-language
  workflows such as Vue, MDX, Astro, Svelte, and Angular templates.
- `codeLensShowLocationsCommandName` requires client-side command support that Zed extensions
  cannot register. Code lenses resolve, but showing their locations remains editor-dependent.

## Development

Install Rust with `rustup`, then run `zed: install dev extension` and select this directory.

After source changes, rebuild the extension from the Extensions page. Run Zed with
`zed --foreground` or use `zed: open log` to inspect extension and language-server errors.

<!-- markdownlint-disable-file MD013 -->
