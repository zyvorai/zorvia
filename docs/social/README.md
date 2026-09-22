# Social assets

Rebuildable cards for README, docs Open Graph, and LinkedIn/X. Story matches the README “What you get” five-step strip (Create → Day-2 → Access → Guard → Audit).

| File | Size | Use |
|------|------|-----|
| `zorvia-share-card.png` | 1200×630 | README hero, docs site `themeConfig.image`, Twitter/OG |
| `zorvia-social-card.jpg` | 1600×900 | LinkedIn / X posts |
| `zorvia-share-card.html` · `zorvia-social-card.html` | sources | Edit HTML, then rebuild |

```bash
./docs/social/build-social-cards.sh
```

Licence wording follows `LICENSE` (Apache-2.0). Version chip tracks `Cargo.toml` / CHANGELOG.
