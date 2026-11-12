# Assets

- `icons/app-icon.png`: original application icon; the source for platform icons.
- `icons/macos/Bitman.icns`: generated macOS icon with standard and Retina sizes.
- `fonts/`: embedded Departure Mono and Sometype Mono faces, licenses, and source
  checksums; see [fonts/README.md](fonts/README.md).

Keep original artwork in `icons/` and platform-specific outputs in their platform
subdirectory. Other resources have their own directories here.

After replacing the source icon, regenerate the macOS file with:

```sh
./scripts/generate-app-icon.sh
```

The script uses macOS's built-in `sips` and `iconutil` tools. It resizes the image
without changing the artwork or transparency. Temporary iconset PNGs are removed
after the `.icns` file is built. Commit both the original PNG and generated ICNS.

`packaging/macos/Info.plist` references the icon through `CFBundleIconFile`.
`scripts/bundle-macos.sh` copies it to the app's `Contents/Resources` directory.
