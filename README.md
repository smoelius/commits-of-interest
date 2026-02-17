# commits-of-interest

Identify commits with meaningful code changes. `commits-of-interest` analyzes the commits between a given revision and HEAD, filtering out changes to non-essential paths (e.g., CI configuration, lock files, tests) and presenting the remaining commits in an interactive TUI for review.

## Usage

```
commits-of-interest <revision>
```

Run `commits-of-interest --help` for more details.

## Filtering

Path components matching any entry in `FILTERED_COMPONENTS` are excluded from diffs. In addition to the hardcoded defaults, you can add extra filtered components by creating a `.filtered_components.txt` file in the root of the repository being analyzed. Each line in the file is treated as a component name to filter out.