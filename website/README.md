# Website

Zola site with a landing page and the live demo (`examples/web`).

```bash
./build-demo.sh     # trunk build into static/demo
zola serve
```

`.github/workflows/website.yml` builds and deploys the site and live demo to GitHub Pages
when a `v*` release tag is pushed, including prereleases such as `vX.Y.Z-rc.1`. The
published version and release-notes link are taken from that tag; pushes to branches do
not deploy the website.
