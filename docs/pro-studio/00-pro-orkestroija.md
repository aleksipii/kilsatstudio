# PRO-ORK — PRO-sarjan orkestroija

Tila: **running** (PR:t yhdistetty, `kilsatstudio1.0` odottaa lupaa) · Lähtö: `main` `d91994c` = `origin/kilsatstudio1.0`

## Tavoite

Kokoaa agenttien P01–P08 työn yhteen ja vie sen läpi ketjun:

**PR → CI vihreä → katselmus → `main` → `kilsatstudio1.0` → julkaisun CI**

Mikään `done`-merkintä ei perustu pelkkiin rajapintoihin tai mockeihin.

## Lähtötiedot

- [README](README.md) (agentit, aallot, kuumien tiedostojen varaukset)
- P01–P08-tehtävätiedostot
- [master-progress.md](../master-progress.md), [release-1.0.md](../release-1.0.md),
  `.github/workflows/` (erityisesti `macos-release-app.yml`, joka ajetaan `main`ille,
  `kilsatstudio1.0`-haaralle ja `v*`-tageille)
- Arkkitehtuuriorkestroija [docs/agents/00-orkestroija.md](../agents/00-orkestroija.md):
  yhteisiä sopimuksia (kello, AudioTimeline, EventLog) ei muuteta ohi sen.

## Sallittu muutosalue

- `docs/pro-studio/**` (tilat, varaukset, yhteenvedot)
- `docs/master-progress.md` (vain yhdistämisen jälkeen)
- Yhdistämiskonfliktien ratkaisu PR-haaroissa. Ei uusia ominaisuuksia; tuotantokoodin
  muutokset tekee omistava agentti.

## Työnkulku

1. **Tilannekuva:** `git fetch origin`, `gh pr list --state open --search "head:feat/pro-"`.
   Päivitä README:n tilataulukko.
2. **Varaukset:** jaa kuumat tiedostot (README) yksi kerrallaan aaltojärjestyksessä.
   Aalto 1 → 2 → 3.
3. **Katselmus PR kerrallaan:**
   - Tarkista, että muutokset ovat agentin sallitulla alueella, ettei uusia riippuvuuksia
     ole ilman päätöstä, että vanhat esimerkit eivät muutu ja että tehtävätiedoston
     lopputulema vastaa koodia.
   - Aja paikallisesti PR-haaralla:
     `cargo fmt --all --check && cargo check --workspace && cargo clippy --workspace --all-targets && cargo test --workspace`
   - Odota CI: `gh pr checks <nro> --watch`. Punainen CI → palauta agentille selkeällä syyllä.
4. **Yhdistäminen `main`iin:** vain kun CI on vihreä ja käyttäjä on antanut
   yhdistämisluvan, joko tässä chatissa tai kirjallisena yleislupana PRO-sarjalle.
   `gh pr merge <nro> --merge` (ei squash-ristiriitoja, ei force-pushia). Yhdistä aalloittain.
   Rebaseta seuraava PR uuden `main`in päälle omistavan agentin kautta, jos konflikteja tulee.
5. **Jälkikirjaus jokaisen yhdistämisen jälkeen** (AGENTS.md):
   - päivitä `docs/master-progress.md` ja yhdistetyn PR:n kuvaus: tehty, oikeasti ajetut
     tarkistukset ja avoimet asiat
   - merkitse agentti `done` README:hen
6. **`kilsatstudio1.0`:**
   - Kun kaikki valitut PR:t ovat `main`issa ja `main`in CI on vihreä, siirrä
     `kilsatstudio1.0` `main`iin **vain fast-forwardina**:
     ```sh
     git fetch origin
     git merge-base --is-ancestor origin/kilsatstudio1.0 origin/main   # pakko onnistua
     git push origin origin/main:refs/heads/kilsatstudio1.0             # ei --force
     ```
   - Jos fast-forward ei onnistu, pysähdy ja kysy käyttäjältä. Älä koskaan force-pushaa.
   - Seuraa `macos-release-app.yml`-ajoa (`gh run watch`) ja kirjaa ZIP-artefaktin nimi
     ja `package_audit.py`:n tulos.
7. **Versio:** kysy käyttäjältä ennen versionumeron muutosta (esim. 1.0.0 → 1.1.0) ja
   ennen `v*`-tagin luomista. Päivitä [release-1.0.md](../release-1.0.md) tai luo uusi
   julkaisumuistio.

## Tehtävät — tarkistuslista

- [x] Tilannekuva ja varaustaulukko ajan tasalla
- [x] Aalto 1 (P01, P02, P04, P06, P07) katselmoitu, CI vihreä, yhdistetty
- [x] Aalto 2 (P03, P05) katselmoitu, CI vihreä, yhdistetty
- [x] Aalto 3 (P08) katselmoitu, CI vihreä, yhdistetty
- [x] `docs/master-progress.md` ja PR-kuvaukset päivitetty jokaisen yhdistämisen jälkeen
- [ ] `kilsatstudio1.0` fast-forwardattu `main`iin, julkaisun CI vihreä, artefakti kirjattu
- [ ] Versio- ja tagipäätös tehty käyttäjän kanssa

## Hyväksymisehdot

- Jokaiselle `done`-agentille on yhdistetty PR, vihreä CI ja omistavan agentin lopputulema.
- Graafisia ominaisuuksia (X-sheet- ja model sheet -paneelit) ei kuvata käsin
  kokeilluiksi ilman oikeaa sovellusajoa.
- Raportissa erotellaan toteutus ja laitteistolla tai mallilla tehty hyväksyntä.

## Toteutunut lopputulema

30.10.2026: kaikki kahdeksan PR:ää (#144–#151) yhdistetty `main`iin aalloittain; `main` = `1cd90a1`.
Orkestroija korjasi #144:n käännösvirheen (`0baa8fb`) ja ratkaisi kuumien tiedostojen
lisäysristiriidat merge-commiteilla (#144, #151, #150, #148). Paikallisesti: fmt, check, clippy
(0 varoitusta) ja test (1226 / 0 / 13) läpi; CI vihreä jokaisella PR:llä ennen yhdistämistä.
Kirjaus: master-progress (PR #152) ja PR-kuvaukset. Avoinna: `kilsatstudio1.0`-fast-forward,
versio/tagi sekä graafiset paneelit kokeilematta oikeassa sovellusajossa.

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-pro-orchestrator. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/00-pro-orkestroija.md. Ota tilannekuva PRO-sarjan PR:istä, katselmoi ja
yhdistä ne aalloittain vihreän CI:n jälkeen, päivitä master-progress ja vie lopuksi
kilsatstudio1.0 fast-forwardina mainiin. Kysy minulta ennen versio- tai tagimuutosta.
```
