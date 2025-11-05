# Desktop Application Icons

This directory should contain the following icon files:

- `32x32.png` - 32x32 pixel icon
- `128x128.png` - 128x128 pixel icon
- `128x128@2x.png` - 256x256 pixel icon (2x)
- `icon.icns` - macOS icon file
- `icon.ico` - Windows icon file

## Generating Icons

Use the Tauri CLI to generate icons from a single source image:

```bash
npm run tauri icon path/to/your-icon.png
```

This will automatically generate all required icon sizes and formats.

## Placeholder Icons

For development, you can use placeholder icons. The app will still build and run without proper icons, though the OS may show default icons instead.
