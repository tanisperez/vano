# Vano

See what fills your disk and clear it safely.

Vano is a native desktop disk analyzer for Linux, macOS and Windows, written in Rust with
[gpui](https://www.gpui.rs/). It scans your home folder, the whole disk or any folder in a
fraction of a second, shows the result as a treemap you can zoom into folder by folder, and
recommends what you can move to the Trash: package caches, build outputs, old installers, idle
`node_modules`, model weights and more. Nothing is removed until you confirm, and everything
goes to the Trash, so it can be put back.

## Build

```sh
make run       # debug build
make release   # optimized build
make test      # all tests
```

See `AGENTS.md` for the architecture and `docs/releasing.md` for packaging.

## License

Apache-2.0, see `LICENSE`.
