# How can we make PRQL successful?

A synthesis of where we're at, and some ideas for focusing the project in a
successful direction for 2H2023.

## Strengths

- Huge developer engagement and excitement — one of the most successful
  open-source launches in 2022, as evidenced by GitHub stars, HackerNews
  reactions, and VC outreach.
- Without rehashing PRQL's logic — this is a promising area for transformation,
  as very few people enjoy SQL — separating the flawed language from the robust
  relational algebra offers significant potential.
- An good institutional design for the problem space — completely open-source,
  devoid of commercial interests; this avoids conflicts of interest, especially
  crucial at the language layer.

## Weaknesses

In summary, we have one major shortcoming: we haven't yet established a flywheel
consisting of:

- Users — using the language, providing feedback on what works and what doesn't.
  They then become evangelists for more users, attracting tools that want to
  integrate PRQL. (Some technical users may even become contributors.)
- Tools — incorporating PRQL into their offerings, broadening PRQL's audience,
  and giving us valuable insights into which language features succeed or fail.
- Developers — building PRQL! Due to the Users & Tools, this feels like an
  impactful and exciting project rather than a personal hobby (although that's
  fine too!).

Specifically, on each of those:

- Very few actual users (compared to stars)
- No outstanding implementation in any data tool. While there are decent
  bindings, it's unclear where to direct someone who simply wants to write PRQL
  and view data. Some attempts (e.g., dbt-prql) faltered on technical grounds.
- Though the Developers aspect of the three-legged stool has consistently been
  the strongest, even here we've seen a decline in engagement:
  - The past few months have had fluctuating levels of contributions outside the
    core team — with a surge in Q1, but slowing since then.
  - Recently, even the core team's pace has decreased (although there's always
    variance, and I'm not suggesting a quick fix by us working harder).

## Finding the Flywheel

The most crucial step is enabling people to use PRQL. Much will follow — we'll
know which features and bugs to tackle, be able to showcase satisfied users, and
so on.

Some options:

- Revisit `dbt`. I had envisioned this as a particularly fruitful integration —
  for both PRQL and dbt.

  - The monkeypatch plugin couldn't technically achieve everything needed.
  - I also failed to have the dbt-core PR merged — it received a brief review
    before being dropped.
  - The dbt team remains interested in merging an improved version of the
    dbt-core plugin (but won't guarantee a merge, which I can understand, though
    makes it a less compelling investment)
  - I'm hesitant to invest more time given this track record but open to it if
    others believe it's a worthwhile bet.

- Enhance language-specific integrations

  - The Jupyter integration is "OK". Python is very popular so if it were
    possible to make it excellent, then it could be great. But can it be great?
    Can we get good editor features in a notebook? I _think_ we can get syntax
    highlighting.
    - TODO: how much integration can we actually do in Jupyter? Could we have an
      LSP?
  - The R extension looks great. I don't have much experience with R and haven't
    explored it extensively. Do others have opinions? Is the R audience enough
    to make a project-wide bet on R?

- Create our own tool. This gives us complete control.

  - But unless it's something exclusive to PRQL, are we likely to build
    something superior to what's available?
  - We're already asking people to bet on PRQL — is asking them to gamble on
    both a new language and a new tool likely to work?
  - Generally, successful things are smaller than we anticipate, not larger.
  - Is there something PRQL can do that SQL can't, which would provide more
    justification for building our own tool? Perhaps something related to
    pipelines and instant feedback (e.g. the playground displaying data results
    is quite impressive and could be expanded)?

- Fork Rill

  - Rill seems to be an especially good match for PRQL — enabling quick
    exploration and immediate results.
  - However, I didn't receive a response from the team in my latest email
    (though I can follow up again). We had a productive Zoom meeting previously.
  - We could consider forking the project if necessary, and then using any
    success there to attempt to merge it to mainline.

Other Considerations

- How can we narrow the project's scope to streamline and concentrate
  development?
  - Limit dialects? Focus solely on DuckDB?
- Would establishing a foundation or securing funding help?
  - I seldom find that a lack of product-market fit can be resolved with
    increased funding. However, if we believe it would be beneficial, it's an
    option worth considering.

## Discussion

[Lutra proposal from @aljaz](https://hackmd.io/@aljazerzen/r1jA61HVh)

- We both think that Lutra & dbt could make good options
- Some questions around do we focus on one vs. diversify bets?
- @aljazerzen particularly keen on Lutra, @max-sixty thinks it's good but
  concerned it's a big jump for users?
- We're going to speak to potential users / industry folks about Lutra, get
  their feedback — @max-sixty to set up
- We could each take Lutra & dbt respectively, see how they go and then coalesce
