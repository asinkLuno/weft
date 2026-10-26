# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "epublib",
#     "markdownify",
# ]
# ///
"""Convert an EPUB into one Markdown file per spine document, using epublib.

Usage:
    uv run tools/epub2md.py BOOK.epub OUTPUT_DIR
"""

from __future__ import annotations

import posixpath
import re
import sys
from pathlib import Path
from urllib.parse import unquote

from epublib import EPUB, ContentDocument, EPUBId
from markdownify import markdownify


def corrected_name(name: str) -> str:
    """Recover a UTF-8 filename that zipfile decoded as cp437."""
    try:
        return name.encode("cp437").decode("utf-8")
    except (UnicodeEncodeError, UnicodeDecodeError):
        return name


def safe_filename(title: str) -> str:
    title = re.sub(r"\s+", "-", title.strip())
    title = title.replace("/", "／").replace("\\", "＼")
    title = re.sub(r'[:*?"<>|]', "", title)
    return title.strip(".-") or "untitled"


def rewrite_images(body, doc_path, resources, images_out) -> None:
    for img in body.find_all("img"):
        src = img.get("src", "")
        if not src or src.startswith(("http:", "https:", "data:")):
            continue
        rel = posixpath.normpath(
            posixpath.join(posixpath.dirname(doc_path), unquote(src))
        )
        resource = resources.get(rel)
        if resource is None:
            print(f"warning: missing image {rel}", file=sys.stderr)
            continue
        name = posixpath.basename(rel)
        dest = images_out / name
        if not dest.exists():
            dest.write_bytes(resource.get_content())
        img["src"] = f"images/{name}"
        img["alt"] = ""


def body_to_markdown(body) -> str:
    md = markdownify(str(body), heading_style="ATX", bullets="-")
    md = md.replace("\r", "")
    lines = ["" if not line.strip() else line for line in md.splitlines()]
    return re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()


def promote_title(md: str, title: str) -> str:
    first, sep, rest = md.partition("\n")
    if re.match(r"^#{1,6}\s+", first):
        first = re.sub(r"^#{1,6}\s+", "# ", first)
        return first + sep + rest
    return f"# {title}\n\n{md}"


def write_cover_art(book, resources, images_out, title: str) -> list[str]:
    cover_meta = book.metadata.get_valued("cover")
    cover_item = (
        book.manifest.get(EPUBId(cover_meta.value)) if cover_meta else None
    )
    if cover_item is None:
        return []
    rel = corrected_name(cover_item.filename)
    resource = resources.get(rel)
    if resource is None:
        return []
    name = posixpath.basename(rel)
    (images_out / name).write_bytes(resource.get_content())
    return [f"# {title}", "", f"![封面](images/{name})", ""]


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} BOOK.epub OUTPUT_DIR")

    source, output = Path(sys.argv[1]), Path(sys.argv[2])
    images_out = output / "images"
    images_out.mkdir(parents=True, exist_ok=True)

    with EPUB(source, generator_tag=False) as book:
        resources = {corrected_name(r.filename): r for r in book.resources}
        title = str(book.metadata.title or "").strip() or "Untitled"
        creator = next(
            (
                str(item.value)
                for item in book.metadata
                if item.name == "creator" and item.value
            ),
            "",
        )

        pages: list[tuple[str, str]] = []  # (label, filename)
        counter = 0

        for index, ref in enumerate(book.spine):
            item = book.manifest.get(ref.idref)
            if item is None or item.media_type != "application/xhtml+xml":
                continue
            doc = book.resources.get(item.filename, ContentDocument)
            body = doc.soup.body
            if body is None:
                continue
            if not body.get_text(strip=True) and not body.find("img"):
                continue  # empty document, e.g. an unused footnotes page

            rewrite_images(body, item.filename, resources, images_out)

            if index == 0:
                # Title page: cover art plus the front-matter images.
                label = "封面"
                lines = write_cover_art(book, resources, images_out, title)
                if not lines:
                    lines = [f"# {title}", ""]
                for img in body.find_all("img"):
                    lines += [f"![]({img['src']})", ""]
                md = "\n".join(lines).rstrip()
            else:
                heading = body.find(["h1", "h2", "h3", "h4"])
                label = (heading.get_text(strip=True) if heading else "") or (
                    doc.get_title().strip()
                )
                label = re.sub(r"\s+", " ", label)
                md = promote_title(body_to_markdown(body), label)

            filename = f"{counter:02d}-{safe_filename(label)}.md"
            (output / filename).write_text(md + "\n", encoding="utf-8")
            pages.append((label, filename))
            counter += 1

        toc = [f"# {title}", ""]
        if creator:
            toc += [f"作者：{creator}", ""]
        toc += [f"- [{label}]({filename})" for label, filename in pages]
        (output / "README.md").write_text("\n".join(toc) + "\n", encoding="utf-8")

        print(f"wrote {len(pages)} chapters to {output}")


if __name__ == "__main__":
    main()
