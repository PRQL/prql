# PRQL Website

A [hugo](https://gohugo.io/) website with PRQL theme, which uses minimal
[water.css](https://watercss.kognise.dev/) styling.

Serve:

```sh
cd website
hugo server
```

To add pages, just add files in `content/` directory.

## Dockerfile

To work on the website, start the Docker container and then use:

```sh
cd website
hugo server --bind 0.0.0.0 -p 3000
# use ^C to abort the hugo server
```
