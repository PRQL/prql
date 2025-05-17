# Syntax highlighting for KSyntaxHighlighting

This is a syntax highlighting file the
[KSyntaxHighlighting](https://invent.kde.org/frameworks/syntax-highlighting)
component used by text editors and integrated development environments such as
[Kate](https://kate-editor.org/), [KWrite](https://apps.kde.org/kwrite/) and
[KDevelop](https://kdevelop.org/).

## Installation

To install for the current user, copy the `prql.xml` file to:

| System               | Path                                                                               |
| -------------------- | ---------------------------------------------------------------------------------- |
| For local user       | `$HOME/.local/share/org.kde.syntax-highlighting/syntax/`                           |
| For Flatpak packages | `$HOME/.var/app/PACKAGE_NAME/data/org.kde.syntax-highlighting/syntax/`             |
| For Snap packages    | `$HOME/snap/PACKAGE_NAME/current/.local/share/org.kde.syntax-highlighting/syntax/` |
| On Windows           | `%USERPROFILE%\AppData\Local\org.kde.syntax-highlighting\syntax\`                  |
| On macOS             | `$HOME/Library/Application Support/org.kde.syntax-highlighting/syntax/`            |

For Flatpak and Snap the PACKAGE_NAME is something like `org.kde.kate`,
`org.kde.kwrite` or `org.kde.kdevelop`.
