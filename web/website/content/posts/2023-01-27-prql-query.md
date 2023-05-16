---
title: Time tracking with pq
date: 2023-01-27
authors: ["Aljaž Mur Eržen"]
layout: article
---

Some time ago, I needed a time-tracking app that would be simple and fast. After
looking into a few heavy web applications, I settled with this one-liner:

```
# time_tracker.sh
echo $(date -u +"%Y-%m-%dT%H:%M:%SZ"),$1 >> ~/time-tracking.csv
```

I've made it a bit more sophisticated, but the core functionality is the same.
The the script is aliased to `tt`, so I can start or stop the timer in any open
terminal by writing:

```
$ tt start
$ tt stop
```

I've prefilled the resulting `~/time-tracking.csv` with a header, so it is ready
to be analyzed.

```
time,action
2023-01-27T09:26:33Z,start
2023-01-27T10:12:50Z,stop
2023-01-27T12:54:04Z,start
2023-01-27T15:12:07Z,stop
```

Now, I'd want to transform this data to show the total duration for each day.

For this I can use [prql-query](https://github.com/PRQL/prql-query), which is a
CLI which can execute PRQL queries against database engines. At the time of
writing it supports duckdb and datafusion, but we can also connect to many other
engines through these two.

But I don't need that today, plain duckdb will do:

```
$ pq --backend=duckdb \
     --from "tt=~/time-tracking.csv" \
     '{here comes the PRQL query below}'
```

```prql
# function declaration that is a wrapper for substr SQL function
let substr = text start len -> s"substr({text}, {start}, {len})"


# start of the pipeline
from tt  # as declared in --from

# compute a few new columns
derive [
    date = substr time 0 11,    # call the substr function to
                                # extract date from column `time`
    prev_action = lag 1 action, # lag column `action`
    prev_time = lag 1 time,     # lag column `time`
]

# pick only rows that correspond to intervals that I want to track
filter action == "stop" and prev_action == "start"

# for each date
group date (
    # sum durations of those intervals
    aggregate [sec = sum s"EXTRACT(EPOCH FROM {time - prev_time})"]
)

# compute more columns
derive [
    hours = substr f"00{sec / (60 * 60)}" 0-2 2,
    minutes = substr f"00{(sec / 60) % 60}" 0-2 2,
    seconds = substr f"00{sec % 60}" 0-2 2,
]

# expose only date and pretty-printed duration
select [
    date,
    duration = f"{hours}:{minutes}:{seconds}"
]
```

When run on the file above, prql-query produces this pretty table:

```
+------------+----------+
| date       | duration |
+------------+----------+
| 2023-01-27 | 03:04:20 |
+------------+----------+
```

The full code of my script can be
[found here](https://github.com/aljazerzen/dotfiles/blob/aebe07e90b5dc86b3974946ded921bdee22e95e8/scripts/tt).

If you want to see how looked when implemented with SQL and SQLite3, see this
[old revision of the file](https://github.com/aljazerzen/dotfiles/blob/fe732ec72e4f4066bfe19041e7d71685dbf69184/scripts/tt).
