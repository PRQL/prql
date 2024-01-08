# Syntax highlighting for GtkSourceView

This is a syntax highlighting file the
[GtkSourceView](https://gitlab.gnome.org/GNOME/gtksourceview) component used by
text editors and integrated development environments such as
[GNOME Text Editor](https://apps.gnome.org/TextEditor/) and
[GNOME Builder](https://apps.gnome.org/Builder/).

## Installation

To install system-wide, copy the `prql.xml` file to:

    /usr/share/gtksourceview-5/language-specs/

To install for the current user, copy the `prql.xml` file to:

    ~/.local/share/gtksourceview-5/language-specs/

## Embedding

To embed it in your GTK application using the `GtkSourceView` widget add it to a
[`LanguageManager`](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/class.LanguageManager.html)
which you add to your
[`Buffer`](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/method.Buffer.set_language.html).
