#!/usr/bin/env python3
"""Generate minimal .xlsx test fixtures using only Python stdlib (zipfile + XML).

.xlsx is a ZIP archive containing XML files in the Office Open XML (OOXML) format.
This script creates valid minimal .xlsx files that calamine can parse.
"""
import zipfile
import os

CONTENT_TYPES = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>'''

RELS = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>'''

WORKBOOK_RELS = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>'''


def make_workbook_xml(sheet_names):
    """Create workbook.xml with given sheet names."""
    sheets = ''
    for i, name in enumerate(sheet_names, 1):
        sheets += f'<sheet name="{name}" sheetId="{i}" r:id="rId{i}"/>'
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
            '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
            f'<sheets>{sheets}</sheets></workbook>')


def make_shared_strings_xml(strings):
    """Create sharedStrings.xml from a list of strings."""
    items = ''
    for s in strings:
        # Escape XML special characters
        s = s.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')
        items += f'<si><t>{s}</t></si>'
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
            f'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
            f'count="{len(strings)}" uniqueCount="{len(strings)}">{items}</sst>')


def make_sheet_xml(rows):
    """Create a worksheet XML from a list of rows.
    Each row is a list of cell values. String values are shared string indices (int),
    numeric values are direct numbers.
    """
    sheet_data = ''
    for row_idx, row in enumerate(rows, 1):
        cells = ''
        for col_idx, cell in enumerate(row):
            # Convert column index to letter (0 -> A, 1 -> B, etc.)
            col_letter = chr(ord('A') + col_idx) if col_idx < 26 else 'A' + chr(ord('A') + col_idx - 26)
            cell_ref = f'{col_letter}{row_idx}'
            if isinstance(cell, str):
                # Shared string reference
                cells += f'<c r="{cell_ref}" t="s"><v>{cell}</v></c>'
            elif isinstance(cell, (int, float)):
                # Numeric value
                cells += f'<c r="{cell_ref}"><v>{cell}</v></c>'
            else:
                cells += f'<c r="{cell_ref}"></c>'
        sheet_data += f'<row r="{row_idx}">{cells}</row>'

    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
            '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
            f'<sheetData>{sheet_data}</sheetData></worksheet>')


def make_xlsx(filepath, sheets):
    """Create an .xlsx file with the given sheets.
    sheets is a list of (name, rows) tuples.
    rows is a list of lists. String values are looked up in the shared strings table.
    """
    # Collect all unique strings
    all_strings = []
    string_map = {}
    for _, rows in sheets:
        for row in rows:
            for cell in row:
                if isinstance(cell, str) and cell not in string_map:
                    string_map[cell] = len(all_strings)
                    all_strings.append(cell)

    # Convert string cells to shared string indices
    sheet_xmls = []
    for _, rows in sheets:
        converted_rows = []
        for row in rows:
            converted_row = []
            for cell in row:
                if isinstance(cell, str):
                    converted_row.append(str(string_map[cell]))
                else:
                    converted_row.append(cell)
            converted_rows.append(converted_row)
        sheet_xmls.append(converted_rows)

    with zipfile.ZipFile(filepath, 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr('[Content_Types].xml', CONTENT_TYPES)
        z.writestr('_rels/.rels', RELS)
        z.writestr('xl/workbook.xml', make_workbook_xml([name for name, _ in sheets]))
        z.writestr('xl/_rels/workbook.xml.rels', WORKBOOK_RELS)
        z.writestr('xl/sharedStrings.xml', make_shared_strings_xml(all_strings))
        for i, rows in enumerate(sheet_xmls, 1):
            z.writestr(f'xl/worksheets/sheet{i}.xml', make_sheet_xml(rows))


os.makedirs('tests/fixtures', exist_ok=True)

# 1. Simple .xlsx with one sheet: name + age columns
make_xlsx('tests/fixtures/test_simple.xlsx', [
    ('Sheet1', [
        ['Name', 'Age'],
        ['Alice', 30],
        ['Bob', 25],
        ['Charlie', 35],
    ]),
])

# 2. Multi-sheet .xlsx: two sheets with different data
make_xlsx('tests/fixtures/test_multi_sheet.xlsx', [
    ('Employees', [
        ['Name', 'Department'],
        ['Alice', 'Engineering'],
        ['Bob', 'Sales'],
    ]),
    ('Products', [
        ['Product', 'Price'],
        ['Widget', 19.99],
        ['Gadget', 29.99],
    ]),
])

# 3. Empty .xlsx (sheet with no data rows)
make_xlsx('tests/fixtures/test_empty.xlsx', [
    ('Empty', []),
])

# 4. Corrupt .xlsx file (not a valid zip)
with open('tests/fixtures/test_corrupt.xlsx', 'w') as f:
    f.write('not an xlsx file')

# 5. .csv file with comma delimiter
with open('tests/fixtures/test_simple.csv', 'w') as f:
    f.write('Name,Age,City\n')
    f.write('Alice,30,New York\n')
    f.write('Bob,25,London\n')
    f.write('Charlie,35,Tokyo\n')

# 6. .csv file with semicolon delimiter
with open('tests/fixtures/test_semicolon.csv', 'w') as f:
    f.write('Name;Age;City\n')
    f.write('Alice;30;New York\n')
    f.write('Bob;25;London\n')

# 7. .csv file with empty lines
with open('tests/fixtures/test_empty_lines.csv', 'w') as f:
    f.write('Name,Age\n')
    f.write('\n')
    f.write('Alice,30\n')
    f.write('\n')
    f.write('Bob,25\n')

print('Fixtures created successfully')
