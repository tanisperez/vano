"""Saves the repository traffic and the release download counts as CSV files.

GitHub keeps traffic data for 14 days only, so the Stats workflow runs this every day and commits
the result to the `stats` branch. Daily series (views, clones) are merged by date, the newest
data winning since the current day is still partial. Snapshots (referrers, paths, downloads,
repository counters) get one set of rows per run date, replaced when the workflow runs twice on
the same day.

Usage: stats.py <output dir>. Reads GITHUB_REPOSITORY, GITHUB_TOKEN (downloads and counters) and
STATS_TOKEN (traffic, needs the Administration read permission). Standard library only.
"""

import csv
import datetime
import json
import os
import sys
import urllib.request

API = "https://api.github.com"


def get(path, token):
    request = urllib.request.Request(f"{API}{path}")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("X-GitHub-Api-Version", "2022-11-28")
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request) as response:
        return json.load(response)


def get_all(path, token):
    items, page = [], 1
    while True:
        batch = get(f"{path}?per_page=100&page={page}", token)
        items.extend(batch)
        if len(batch) < 100:
            return items
        page += 1


def read_rows(path):
    if not os.path.exists(path):
        return []
    with open(path, newline="") as file:
        return list(csv.DictReader(file))


def write_rows(path, fields, rows):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def merge_daily(path, series):
    """Merges a views or clones series into the file, keyed by date."""
    by_date = {row["date"]: row for row in read_rows(path)}
    for day in series:
        date = day["timestamp"][:10]
        by_date[date] = {"date": date, "count": day["count"], "uniques": day["uniques"]}
    write_rows(path, ["date", "count", "uniques"], [by_date[d] for d in sorted(by_date)])


def save_snapshot(path, fields, today, rows):
    """Replaces the rows of `today` in a snapshot file with `rows`."""
    kept = [row for row in read_rows(path) if row["date"] != today]
    write_rows(path, ["date", *fields], kept + [{"date": today, **row} for row in rows])


def main():
    out = sys.argv[1]
    repo = os.environ["GITHUB_REPOSITORY"]
    token = os.environ.get("GITHUB_TOKEN")
    stats_token = os.environ.get("STATS_TOKEN")
    today = datetime.datetime.now(datetime.timezone.utc).date().isoformat()

    releases = get_all(f"/repos/{repo}/releases", token)
    downloads = [
        {"tag": release["tag_name"], "asset": asset["name"], "downloads": asset["download_count"]}
        for release in releases
        for asset in release["assets"]
    ]
    save_snapshot(f"{out}/downloads.csv", ["tag", "asset", "downloads"], today, downloads)

    info = get(f"/repos/{repo}", token)
    counters = {
        "stars": info["stargazers_count"],
        "forks": info["forks_count"],
        "watchers": info["subscribers_count"],
    }
    save_snapshot(f"{out}/repo.csv", list(counters), today, [counters])

    if not stats_token:
        print("STATS_TOKEN is not set, skipping the traffic data.")
        return
    traffic = f"/repos/{repo}/traffic"
    merge_daily(f"{out}/traffic/views.csv", get(f"{traffic}/views", stats_token)["views"])
    merge_daily(f"{out}/traffic/clones.csv", get(f"{traffic}/clones", stats_token)["clones"])
    for name, key in (("referrers", "referrer"), ("paths", "path")):
        rows = [
            {key: item[key], "count": item["count"], "uniques": item["uniques"]}
            for item in get(f"{traffic}/popular/{name}", stats_token)
        ]
        save_snapshot(f"{out}/traffic/{name}.csv", [key, "count", "uniques"], today, rows)


if __name__ == "__main__":
    main()
