#!/usr/bin/env python3
"""Generate minimal .epub test fixtures using only Python stdlib (zipfile + XML).

EPUB structure:
  mimetype           (uncompressed, "application/epub+zip")
  META-INF/
    container.xml    (points to the OPF file)
  OEBPS/
    content.opf      (metadata + manifest + spine)
    toc.ncx          (EPUB 2 table of contents)
    chapterN.xhtml   (XHTML content files)
"""
import zipfile
import os

MIMETYPE = "application/epub+zip"

CONTAINER_XML = '''<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>'''


def make_opf(title, author, chapters, toc_id="ncx"):
    """Build content.opf with given metadata and manifest/spine entries.

    Args:
        title: Book title
        author: Book author
        chapters: List of (id, href) tuples for XHTML files
        toc_id: ID of the NCX/nav file in manifest
    """
    manifest_items = ""
    spine_items = ""
    for ch_id, ch_href in chapters:
        manifest_items += f'    <item id="{ch_id}" href="{ch_href}" media-type="application/xhtml+xml"/>\n'
        spine_items += f'    <itemref idref="{ch_id}"/>\n'
    # Add NCX entry
    manifest_items += f'    <item id="{toc_id}" href="toc.ncx" media-type="application/x-dtbncx+xml"/>\n'

    return f'''<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
    <dc:identifier id="BookId">urn:uuid:test-001</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
{manifest_items}  </manifest>
  <spine toc="{toc_id}">
{spine_items}  </spine>
</package>'''


def make_ncx(title, nav_points):
    """Build toc.ncx (EPUB 2 table of contents).

    Args:
        title: Book title for the navMap
        nav_points: List of (label, src) tuples
    """
    nav_xml = ""
    for i, (label, src) in enumerate(nav_points):
        nav_xml += f'    <navPoint id="nav{i}" playOrder="{i+1}">\n'
        nav_xml += f'      <navLabel><text>{label}</text></navLabel>\n'
        nav_xml += f'      <content src="{src}"/>\n'
        nav_xml += '    </navPoint>\n'

    return f'''<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dt:uid" content="urn:uuid:test-001"/>
    <meta name="dt:depth" content="1"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
{nav_xml}  </navMap>
</ncx>'''


def make_xhtml(body_content, title="Chapter"):
    """Build a minimal XHTML 1.1 document."""
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>{title}</title>
</head>
<body>
{body_content}
</body>
</html>'''


def make_epub(filepath, opf_content, toc_content, xhtml_files):
    """Create an EPUB ZIP file.

    Args:
        filepath: Output .epub path
        opf_content: content.opf XML string
        toc_content: toc.ncx XML string
        xhtml_files: Dict of {filename: xhtml_content}
    """
    with zipfile.ZipFile(filepath, 'w', zipfile.ZIP_DEFLATED) as z:
        # mimetype must be first and uncompressed
        z.writestr('mimetype', MIMETYPE, compress_type=zipfile.ZIP_STORED)
        z.writestr('META-INF/container.xml', CONTAINER_XML)
        z.writestr('OEBPS/content.opf', opf_content)
        z.writestr('OEBPS/toc.ncx', toc_content)
        for filename, content in xhtml_files.items():
            z.writestr(f'OEBPS/{filename}', content)


os.makedirs('tests/fixtures', exist_ok=True)

# 1. Simple single-chapter EPUB
simple_body = '''
<h1>Introduction</h1>
<p>This is the first paragraph of the test book.</p>
<p>This is the second paragraph.</p>
'''
simple_xhtml = make_xhtml(simple_body, "Introduction")
simple_opf = make_opf("Test Book", "Test Author", [("ch1", "chapter1.xhtml")])
simple_toc = make_ncx("Test Book", [("Introduction", "chapter1.xhtml")])
make_epub('tests/fixtures/test_simple.epub', simple_opf, simple_toc,
          {"chapter1.xhtml": simple_xhtml})

# 2. Multi-chapter EPUB with title hierarchy
multi_ch1_body = '''
<h1>Chapter One</h1>
<p>Content of chapter one.</p>
'''
multi_ch2_body = '''
<h1>Chapter Two</h1>
<p>Content of chapter two.</p>
<h2>Section 2.1</h2>
<p>Subsection content.</p>
'''
multi_ch3_body = '''
<h1>Chapter Three</h1>
<p>Final chapter content.</p>
'''
multi_xhtml = {
    "chapter1.xhtml": make_xhtml(multi_ch1_body, "Chapter One"),
    "chapter2.xhtml": make_xhtml(multi_ch2_body, "Chapter Two"),
    "chapter3.xhtml": make_xhtml(multi_ch3_body, "Chapter Three"),
}
multi_opf = make_opf("Multi Chapter Book", "Test Author", [
    ("ch1", "chapter1.xhtml"),
    ("ch2", "chapter2.xhtml"),
    ("ch3", "chapter3.xhtml"),
])
multi_toc = make_ncx("Multi Chapter Book", [
    ("Chapter One", "chapter1.xhtml"),
    ("Chapter Two", "chapter2.xhtml"),
    ("Chapter Three", "chapter3.xhtml"),
])
make_epub('tests/fixtures/test_multi.epub', multi_opf, multi_toc, multi_xhtml)

# 3. HTML tags EPUB (bold, italic, links, lists)
html_body = '''
<h1>HTML Test Chapter</h1>
<p>This has <b>bold text</b> and <i>italic text</i>.</p>
<p>Here is a <a href="https://example.com">hyperlink</a> in text.</p>
<ul>
<li>First item</li>
<li>Second item</li>
</ul>
<p>A paragraph with <strong>strong emphasis</strong> and <em>emphasized text</em>.</p>
'''
html_xhtml = make_xhtml(html_body, "HTML Test Chapter")
html_opf = make_opf("HTML Tags Book", "Test Author", [("ch1", "chapter1.xhtml")])
html_toc = make_ncx("HTML Tags Book", [("HTML Test Chapter", "chapter1.xhtml")])
make_epub('tests/fixtures/test_html.epub', html_opf, html_toc,
          {"chapter1.xhtml": html_xhtml})

# 4. Corrupt EPUB (just text, not a valid ZIP)
with open('tests/fixtures/test_corrupt.epub', 'w') as f:
    f.write('not a valid epub file')

print('EPUB fixtures created successfully')
