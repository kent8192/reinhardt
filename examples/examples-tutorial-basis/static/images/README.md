# Static Images Directory

Place your poll-related images here. These images can be referenced in your
components using `resolve_static()`.

## Example Images

- `poll-icon.svg` - Icon displayed next to poll questions
- `logo.png` - Application logo (optional)
- `avatars/` - User avatar images (optional)

## Usage in Components

```rust
use reinhardt::pages::static_resolver::resolve_static;

page!(|| {
    img {
        src: resolve_static("images/poll-icon.svg"),
        alt: "Poll Icon"
    }
})()
```

## Note on SVG Icon

For this tutorial, we provide a simple bar chart SVG icon (`poll-icon.svg`).
You can replace it with your own custom icon or use any other SVG graphic.

The provided icon is a simple placeholder showing 4 vertical bars in different
heights, representing poll data visualization.
