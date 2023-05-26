# Misc

This file contains many different queries I rewrote from various languages with
intention of finding examples of where PRQL could be improved.

A SQL query to find all stubs in email addresses of accounts associated with
some prospect list in a MariaDB of a CRM system.

```prql
# TODO: this table should have a column `part` with values 1..5,
# but such data declaration is not yet supported, see #286
let parts = (
    from seq_1_to_5
)

from pl=prospect_lists_prospects
filter prospect_list_id == 'cc675eee-8bd1-237f-be5e-622ba511d65e'
join a=accounts (a.id == pl.related_id)
join er=email_addr_bean_rel (er.bean_id == a.id && er.primary_address == '1')
join ea=email_addresses (ea.id == er.email_address_id)
select ea.email_address
derive prefix = s"regexp_replace(SUBSTRING_INDEX({email_address}, '@', 1), '[.0-9-_:]+', '.')"
derive stub = s"SUBSTRING_INDEX(SUBSTRING_INDEX({prefix}, '.', part), '.', -1)"
select {email_address, stub}
```

European football clubs with ratings for each year. We want to normalize each
year separately.

```prql
from club_ratings
filter rating != null
# TODO: this is real ugly. `average rating` should not require parenthesis
# TODO: why cannot we put comments in group's pipeline?
group year (
    derive {rating_norm = rating - (average rating) / (stddev rating)}
)
```
