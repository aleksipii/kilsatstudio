# Macin tuotantojulkaisu ja fyysinen hyväksyntä

Tilanne: allekirjoitus- ja notarisointipolku on toteutettu, mutta sitä ei ole
suoritettu hyväksytyllä Apple-identiteetillä tässä työssä. Tavallinen CI-paketti
on edelleen ad-hoc-allekirjoitettu esikatselu. Allekirjoitus ei hyväksy aineistoja,
ääniä, lisenssejä tai fyysisen Macin toimivuutta.

## Developer ID ja notarisointi

Käytä omaa Macia, Developer ID Application -varmennetta ja valmiiksi luotua
notarytool-avaimenperäprofiilia. Älä lähetä varmenteita, salasanoja tai avaimia
keskusteluun tai tallenna niitä repositoryyn. Applen ohjeet:
https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
https://developer.apple.com/documentation/security/customizing-the-notarization-workflow

Rakenna release-profiilin Apple Silicon -sovellus, GPU-aputyökalu sekä kameran
Swift-aputyökalu nykyisen macOS-preview-työnkulun ohjeilla. Suorita sitten:

```sh
python3 scripts/developer_release.py --binary target/aarch64-apple-darwin/release/studio-desktop --gpu-probe target/aarch64-apple-darwin/release/examples/gpu_probe --head-pose dist/head_pose --commit FULL_COMMIT_SHA --identity 'Developer ID Application: NAME (TEAMID1234)' --team TEAMID1234 --notary-profile PROFILE --output dist/new-release.zip --report dist/new-release-report.json --allow-notarization
```

Työkalu allekirjoittaa sisäkkäiset ohjelmat ensin, käyttää hardened runtimea,
rajaa kameraentitlementin tarvittaviin ohjelmiin, tarkistaa tiimin, odottaa Applen
Accepted-tuloksen, nidottaa lipukkeen ja tarkistaa Gatekeeper-arvion ennen
lopullisen ZIPin luontia. Vanhoja tuloksia ei korvata. Lopullinen hash lasketaan
nidonnan jälkeen. Aja nykyiset package_audit/build_evidence/license-raportit juuri
näistä lopullisista tavuista. Jakeluhyväksyntä ei synny automaattisesti.

## Fyysisen Macin testi

Käytä [nykyistä laitetestausmenettelyä](workstreams/mac-device-acceptance.md) ja
scripts/mac_acceptance_report.py:n rakennukseen sidottuja 24 skenaariota.
Merkitse suorittamattomat kohdat not_tested; CI:n käännös ei ole GUI-testi.

1. Säilytä ladattu ZIP, SHA-256 ja BUILD-INFO. Käynnistä Finderistä ja testaa
   uuden jakson/kohtauksen luonti, tallennus, uudelleenavaus sekä Kumoa/Tee uudelleen.
2. Pysäytä toisto, vedä hahmoa kahdessa aikapisteessä ja tarkista liike. Tarkista
   ikkunoiden skaalaus, näppäinoikotiet ja käyttöapu todellisessa sovelluksessa.
3. Testaa jo asennettu, käyttöoikeuksiltaan hyväksytty suomen Piper-ääni ilman
   verkkoyhteyttä. Kuuntele oikeasta nimetyistä äänilaitteesta; testiääni ei kelpaa.
4. Testaa kameran luvan hyväksyntä ja kielto, Seuraa/Nauhoita/Pysäytä/Käytä,
   koko nauhoituksen Kumoa sekä kontekstinvaihdon peruutus. Varmista kameran
   sammuminen sovelluksen sulkemisen jälkeen. Kuvia tai ääntä ei saa tallentua.
5. Aja scripts/benchmark_animation_pipeline.py todellisella 30–60 sekunnin,
   Full HD 24 fps -projektilla ja oikealla puheella. Mittaa kylmä ja lämmin ajo
   erikseen. --physical-mac on operaattorin vakuutus; havaittu CI/VM ei läpäise.
6. Kuuntele ja katso lopullinen video: suun ajoitus, eleet, rajaukset, vitsin
   luettavuus ja äänen tasot. Säilytä tarvittavat luvalliset todisteet ja
   manuaaliset huulisynkka-annotaatiot erillään mallipainoista.

Vain todelliset laitehavainnot ja hyväksytyt oikeus-/allekirjoitustiedot voivat
sulkea nämä portit. Tämä ohje tai testityökalun olemassaolo ei sulje niitä.
