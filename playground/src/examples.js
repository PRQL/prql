const examples = {
  "introduction.prql": [
    "arrow",
    `from invoices
filter invoice_date >= @1970-01-16
derive [                        # This adds columns
  transaction_fees = 0.8,
  income = total - transaction_fees  # Columns can use other columns
]
filter income > 1     # Transforms can be repeated.
group customer_id (   # Use a nested pipeline on each group
  aggregate [         # Aggregate each group to a single row
    average total,
    sum_income = sum income,
    ct = count,
  ]
)
sort [-sum_income]    # Decreasing order
take 10               # Limit to top 10 spenders
join c=customers [==customer_id]
derive name = f"{c.last_name}, {c.first_name}"
select [              # Select only these columns
  c.customer_id, name, sum_income
]
derive db_version = s"version()" # S-string, escape hatch to SQL
`,
  ],

  "let-table-0.prql": [
    "arrow",
    `let soundtracks = (
  from playlists
  filter name == 'TV Shows'
  join pt=playlist_track [==playlist_id]
  select pt.track_id
)

let high_energy = (
  from genres
  filter name == 'Rock And Roll' or name == 'Hip Hop/Rap'
)

from t=tracks

# anti-join soundtracks
join side:left s=soundtracks [==track_id]
filter s.track_id == null

# limit to kicker genres
join g=high_energy [==genre_id]

# format output
select [t.track_id, track = t.name, genre = g.name]
take 10
`,
  ],

  "artists-0.prql": [
    "arrow",
    `from tracks
select [album_id, name, unit_price]
sort [-unit_price, name]
group album_id (
    aggregate [
    track_count = count,
    album_price = sum unit_price
    ]
)
join albums [==album_id]
group artist_id (
    aggregate [
    track_count = sum track_count,
    artist_price = sum album_price
    ]
)
join artists [==artist_id]
select [artists.name, artist_price, track_count]
sort [-artist_price]
derive avg_track_price = artist_price / track_count
`,
  ],
};
export default examples;
