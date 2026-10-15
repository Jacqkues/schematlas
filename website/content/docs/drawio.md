---
title: draw.io
description: Schematlas reads draw.io pages and stores its notes as cell properties that draw.io shows under Edit Data.
---

## Files and pages

Files ending in `.drawio` or `.dio` are read, page by page: `infra.drawio#0` is the first page, `infra.drawio#1` the second. `.drawio.svg` files are shown read-only. Both plain and compressed pages are supported, and a compressed page is written back compressed.

## Annotations as cell properties

draw.io lets every shape carry custom properties (right-click → *Edit Data*). Schematlas uses them, with a `schematlas-` prefix:

| Property | Holds |
| --- | --- |
| `schematlas-code` | Code references, one per line |
| `schematlas-note` | Markdown note |
| `schematlas-kind` | Node kind |
| `schematlas-tags` | Comma-separated tags |
| `schematlas-title` | Title |
| `schematlas-color`, `schematlas-commit`, … | Any other key |

A plain shape is wrapped in a `UserObject` the first time it is annotated, which is how draw.io itself stores properties:

```xml
<UserObject label="Orders API" schematlas-code="src/api/checkout.rs:18-64"
            schematlas-note="Validates carts" id="api">
  <mxCell style="rounded=1;" vertex="1" parent="1">
    <mxGeometry x="120" y="80" width="160" height="60" as="geometry"/>
  </mxCell>
</UserObject>
```

Annotations about the whole page live on its root cell.

## Layout

draw.io files keep their own geometry. Moving a node in Schematlas moves it in the file, and **Tidy up** writes the computed layout into the file. No `.schematlas/layout/` file is created for draw.io.

Everything else in the file — styles, other pages, metadata — is kept byte for byte.
