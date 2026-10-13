# TheDiscDB API answers

TheDiscDB's answers for the discs whose scan logs are pinned in `../thediscdb/`: one `dischash-<content hash>.json`
(`https://thediscdb.com/api/dischash/<hash>`, one object) and one `discid-<disc ID>.json`
(`https://thediscdb.com/api/discid/<id>`, an array) per disc, 18 files in all, saved unchanged by
`scripts/phase0/fetch-thediscdb-fixtures.sh` on 2026-10-04 from TheDiscDB's public, anonymous API.

They are live answers, not pinned ones: unlike the scan logs, which come from a fixed commit of TheDiscDB's data
repository, the API serves the database as it is on the day of the request, so fetching again may give different
answers. They show the answers' shape as it was on 2026-10-04 (spec 8.1) and are test fixtures, not reference data.

Source and terms: the records are TheDiscDB's data (https://thediscdb.com), which its data repository
(https://github.com/TheDiscDb/data) publishes under the MIT licence; its licence text is in `../thediscdb/LICENSE`.
No separate licence came with the API's answers. They are kept here unchanged, with this attribution.

The owner's own discs' answers are not here: each recording keeps its own (`corpus/owner/<slug>/thediscdb-hash.json`
and `thediscdb-discid.json`).
