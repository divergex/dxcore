#!/usr/bin/env python3
"""
Look up ESEF/XBRL filings for a company on filings.xbrl.org and print
a formatted table.

Usage:
    python3 xbrl_filings_lookup.py ASML
    python3 xbrl_filings_lookup.py "Interactive Brokers"
    python3 xbrl_filings_lookup.py ASML --limit 10
"""

import argparse
import json
import sys
import urllib.parse
import urllib.request

API_URL = "https://filings.xbrl.org/api/filings"

HEADERS = {
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:150.0) Gecko/20100101 Firefox/150.0",
    "Accept": "application/json, text/javascript, */*; q=0.01",
    "Accept-Language": "en-GB,en;q=0.9",
    "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
    "X-Requested-With": "XMLHttpRequest",
    "Connection": "keep-alive",
    "Referer": "https://filings.xbrl.org/",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-origin",
}


def build_url(query: str, limit: int) -> str:
    # Reproduces the exact filter structure the filings.xbrl.org frontend
    # itself sends (captured via browser devtools), unmodified. Do not trim
    # the "or" clauses or drop the UAIFRS exclusion -- both were present in
    # the confirmed-working request.
    filt = [
        {
            "or": [
                {"name": "entity.name", "op": "ilike", "val": f"%{query}%"},
                {"name": "entity.identifier", "op": "eq", "val": query},
                {"name": "language.name", "op": "ilike", "val": f"{query}%"},
                {"name": "sha256", "op": "eq", "val": query},
            ]
        },
        {"name": "input_filing.filing_source.program", "op": "ne", "val": "UAIFRS"},
    ]
    params = {
        "include": "entity,language",
        "filter": json.dumps(filt),
        "sort": "-date_added",
        "page[size]": str(limit),
        "page[number]": "1",
    }
    return f"{API_URL}?{urllib.parse.urlencode(params)}"


def fetch(url: str) -> dict:
    req = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(req, timeout=20) as resp:
        return json.loads(resp.read().decode("utf-8"))


def index_included(payload: dict):
    """Build lookup tables for included entity/language resources by (type, id)."""
    lookup = {}
    for item in payload.get("included", []):
        lookup[(item.get("type"), item.get("id"))] = item.get("attributes", {})
    return lookup


def get_related(rel, lookup, rel_type):
    """Resolve a to-one relationship to its attributes dict, or {}."""
    data = rel.get(rel_type, {}).get("data")
    if not data:
        return {}
    return lookup.get((data.get("type"), data.get("id")), {})


def rows_from_payload(payload: dict, base_url: str = "https://filings.xbrl.org"):
    lookup = index_included(payload)
    rows = []
    for item in payload.get("data", []):
        attrs = item.get("attributes", {})
        rels = item.get("relationships", {})

        entity_attrs = get_related(rels, lookup, "entity")
        language_attrs = get_related(rels, lookup, "language")

        entity_name = entity_attrs.get("name", "—")
        country = attrs.get("country", entity_attrs.get("country", "—")) or "—"
        language = language_attrs.get("name", attrs.get("language", "—")) or "—"
        period_end = attrs.get("period_end", "—") or "—"
        date_added = attrs.get("date_added", "—") or "—"

        # "View" link — the filings.xbrl.org viewer URL for this filing
        filing_id = item.get("id", "")
        view_url = f"{base_url}/{filing_id}" if filing_id else "—"

        rows.append([entity_name, language, country, period_end, date_added, view_url])
    return rows


def print_table(rows, headers):
    if not rows:
        print("No filings found.")
        return

    widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(str(cell)))

    def fmt_row(cells):
        return "  ".join(str(c).ljust(widths[i]) for i, c in enumerate(cells))

    print(fmt_row(headers))
    print("  ".join("-" * w for w in widths))
    for row in rows:
        print(fmt_row(row))


def main():
    parser = argparse.ArgumentParser(
        description="Look up ESEF/XBRL filings by ticker or entity name."
    )
    parser.add_argument(
        "query", help="Ticker, LEI, or (partial) company name to search for"
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=20,
        help="Max number of filings to return (default 20)",
    )
    args = parser.parse_args()

    url = build_url(args.query, args.limit)

    try:
        payload = fetch(url)
    except urllib.error.HTTPError as e:
        print(f"HTTP error {e.code}: {e.reason}", file=sys.stderr)
        sys.exit(1)
    except urllib.error.URLError as e:
        print(f"Network error: {e.reason}", file=sys.stderr)
        sys.exit(1)

    rows = rows_from_payload(payload)
    headers = ["Entity", "Language", "Country", "Period Ending", "Date Added", "View"]
    print_table(rows, headers)
    print(f"\n{len(rows)} filing(s) found for '{args.query}'.")


if __name__ == "__main__":
    main()
