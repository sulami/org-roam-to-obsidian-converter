# Convert

This is a small tool that helps convert org-roam note collections into Obsidian vaults. To that end, it inspects the org-roam database to extract all nodes (including subtrees with IDs), and renders each of them to a separate markdown file. This means that subtrees can end up in several notes, but I haven't found an easy way to avoid that yet.

This tool will write to the org files to modify the links, patching them from org-roam IDs to markdown files. Make sure to backup your notes first.

The actual conversion is done via Emacs export.

## Using

Build it using

```sh
cargo build --release
```

and run the resulting binary

```sh
target/release/convert --db ~/.emacs.d/org-roam.db --target-dir /Users/sulami/Documents/Obsidian
```

# Limitations

As noted above, this writes to the source files.

If there are subtrees with IDs, those subtrees will be present in their parent notes, and in their own notes.

Tags are not included, because there are several different ways of assigning tags to an org-roam node, and Emacs does not export them.

In general, metadata is not included, or sometimes rendered in non-ideal ways. For example clocking data is missing.

It's quite slow. For reasons this tool call up a fresh Emacs instance to export every single file, which means export time depends hugely on Emacs startup time. It does load `init.el` to ensure all required packages are there e.g. for syntax highlighting source blocks.

There are a few export edge cases, which are arguably Emacs' fault. I've had Emacs fail to export a file with a rather large table for example. Broken links also need to be fixed by hand.

Because of Obsidian's limitations, node titles are patched to e.g. remove slashes. I might not be covering all offenders, just enough for my own notes.

On the bright side, if an export fails this tool prints out some diagnostics and aborts, but will skip over already exported files on the next run, so no time is lost.