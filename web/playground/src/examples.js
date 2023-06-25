const examples = {
  "introduction.prql": [
    "sql",
    `from invoices                        # PRQL query begins with a table
derive {                             # "derive" adds columns
  transaction_fees = 0.8,            # "=" sets a column name
  income = total - transaction_fees  # Calculations can use other column names
}                                    # "#" starts a comment - even at start of line
filter income > 1                    # "filter" replaces both of SQL's WHERE & HAVING
filter invoice_date >= @1970-01-16   # Clear date syntax
group customer_id (                  # "group" performs the pipeline in (...) on each group
  aggregate {                        # "aggregate" reduces each group to a single value
    average total,                   # get the average of the total for each customer_id
    sum_income = sum income,         # ... and the total of income
    ct = count customer_id,          # ... and the count of items
  }
)
join c=customers (==customer_id)     # join on "customer_id" from both tables
derive name = f"{c.last_name}, {c.first_name}" # F-strings like Python
derive db_version = s"version()"     # S-string give escape hatch to SQL
select {                             # Select certain columns
  c.customer_id, name, sum_income, ct, db_version,
}                                    # trailing comma ignored
sort {-sum_income}                   # "sort" sorts the result; "-" is decreasing order
take 1..10                           # Limit to a range - could also be "take 10"
#
# "output.sql" tab at right shows the SQL generated from this PRQL query
# "output.arrow" tab shows the result of the query
`,
  ],

  "let-table-0.prql": [
    "sql",
    `let soundtracks = (
  from playlists
  filter name == 'TV Shows'
  join pt=playlist_track (==playlist_id)
  select pt.track_id
)

let high_energy = (
  from genres
  filter name == 'Rock And Roll' || name == 'Hip Hop/Rap'
)

from t=tracks

# anti-join soundtracks
join side:left s=soundtracks (==track_id)
filter s.track_id == null

# limit to kicker genres
join g=high_energy (==genre_id)

# format output
select {t.track_id, track = t.name, genre = g.name}
take 10
`,
  ],

  "artists-0.prql": [
    "sql",
    `from tracks
select {album_id, name, unit_price}
sort {-unit_price, name}
group album_id (
    aggregate {
    track_count = count name,
    album_price = sum unit_price
    }
)
join albums (==album_id)
group artist_id (
    aggregate {
    track_count = sum track_count,
    artist_price = sum album_price
    }
)
join artists (==artist_id)
select {artists.name, artist_price, track_count}
sort {-artist_price}
derive avg_track_price = artist_price / track_count
`,
  ],
};
export default examples;
