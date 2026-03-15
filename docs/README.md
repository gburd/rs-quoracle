# Quoracle Documentation

This directory contains the mdBook documentation source for Quoracle.

## Building Locally

### Prerequisites

Install mdBook:
```bash
cargo install mdbook
```

Or use Nix:
```bash
nix develop
```

### Build and Serve

```bash
# Build the book
mdbook build

# Serve with live reload
mdbook serve
```

Then visit http://localhost:3000

## Structure

```
docs/
├── src/           # Markdown source files
│   ├── SUMMARY.md # Table of contents
│   ├── introduction.md
│   ├── quick-start.md
│   ├── examples.md
│   ├── performance.md
│   └── api.md
├── book/          # Generated output (gitignored)
└── README.md      # This file
```

## Editing

1. Edit Markdown files in `docs/src/`
2. Test with `mdbook serve`
3. Commit changes
4. GitHub Actions will automatically build and deploy

## Deployment

Documentation is automatically deployed to GitHub Pages on push to `main`:
- Workflow: `.github/workflows/docs.yml`
- Site URL: https://gregburd.github.io/quoracle

## Configuration

mdBook configuration in `book.toml`:
- Site URL and repository links
- Code syntax highlighting
- Theme and styling

## Writing Tips

- Keep examples runnable and tested
- Use proper code blocks with language tags
- Link to docs.rs for API reference
- Keep page titles concise
- Use `#` for page title, `##` for sections

## mdBook Resources

- [mdBook Guide](https://rust-lang.github.io/mdBook/)
- [Markdown Syntax](https://rust-lang.github.io/mdBook/format/markdown.html)
- [Theme Customization](https://rust-lang.github.io/mdBook/format/theme/)
