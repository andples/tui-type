#!/usr/bin/env python3
# Writes a fake year of history for trying the stats screen:
#   scripts/fake-history.py /tmp/try/history.jsonl
import json, random, sys, datetime as dt
random.seed(11); out = open(sys.argv[1], "w")
now = dt.datetime.now().astimezone()
today = now.replace(hour=9, minute=0, second=0, microsecond=0)
if today > now: today -= dt.timedelta(hours=8)
wpm = 58.0
for back in range(330, -1, -1):
    day = today - dt.timedelta(days=back)
    p = 0.35 + 0.4 * (1 - back / 330) + (0.15 if day.weekday() >= 5 else 0)
    if 120 < back < 135: p = 0.0
    if back <= 9: p = 1.0
    if random.random() > p: continue
    wpm += 0.06
    for i in range(random.choice([1, 1, 2, 3, 4, 6, 9]) if back > 9 else random.randint(2, 8)):
        ts = day + dt.timedelta(minutes=13 * i + random.randint(0, 9))
        if ts > now: continue
        secs, w = random.choice([15, 30, 30, 60]), max(20.0, random.gauss(wpm, 6))
        out.write(json.dumps({"schema": 2, "mode": {"time": secs}, "language": "english",
            "ts": ts.astimezone(dt.timezone.utc).isoformat().replace("+00:00", "Z"),
            "punctuation": False, "numbers": False, "wpm": w, "raw": w + 4,
            "acc": random.uniform(91, 99), "consistency": random.uniform(65, 85),
            "chars": {"correct": int(w * secs / 12), "incorrect": 3, "extra": 0, "missed": 0},
            "duration_s": float(secs)}) + "\n")
