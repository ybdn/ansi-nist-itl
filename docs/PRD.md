# PRD — ansi-nist-itl

> **Statut** : référence, figé au 2026-11-10 (état de la version 1.0.1) ; remplace le brouillon v0.1 du 2026-10-20. Les évolutions futures se décident dans les issues du dépôt.
> **Date** : 2026-11-10
> **Auteur** : ybdn
> **Dépôt et crate** : `ansi-nist-itl` — nom disponible sur crates.io au 2026-10-20 ; crate publiée depuis la 1.0.0 (2026-10-23)
> **Objet** : définir le périmètre de la bibliothèque Rust de lecture/écriture ANSI/NIST-ITL, qui remplace la bibliothèque Python la référence et sert de fondation à NIST Studio (Rust).

**Légende — colonne « Origine »**

| Code | Signification |
|------|---------------|
| **REF** | Capacité présente dans la référence → parité attendue |
| **NEW** | Absente de la référence → amélioration |

**Colonne « Prio. »** : `M` (Must, 1.0) / `S` (Should) / `C` (Could) / `W` (Won't).
Colonne « Statut » : état dans la version 1.0.1 (*Livré*, *Partiel*, *Non livré*) ; elle renvoie à l'issue ou à la décision de §9 quand quelque chose reste à faire.

---

## 1. Contexte

- La v1 de NIST Studio (Python) délègue tout le traitement des fichiers NIST à la référence (v0.6, ~1 900 lignes).
- La réécriture de NIST Studio en Rust nécessite un équivalent Rust. À notre connaissance, il n'existe pas de bibliothèque ANSI/NIST-ITL Rust complète et maintenue.
- La bibliothèque est développée **dans un dépôt séparé et publiée en open source**, pour :
  - la partager (intégrateurs, laboratoires, autres outils de l'écosystème YBDN comme AFIS Console) ;
  - imposer une API propre, indépendante de toute interface ;
  - lui donner son propre cycle de versions, sa CI et sa documentation (docs.rs).

NIST Studio **dépend** de la crate (dépendance Cargo), il ne la forke pas :

```
ansi-nist-itl (public, crates.io)  ◄── dépendance Cargo ──  nist-studio-rust (privé)
                                   ◄──                      autres outils (AFIS Console, CLI…)
```

## 2. Objectifs

| # | Objectif | Indicateur |
|---|----------|------------|
| O1 | Parité fonctionnelle avec la référence | Toutes les exigences d'origine REF couvertes ; doctests la référence portés au vert |
| O2 | Fidélité | Aller-retour lecture → écriture identique octet pour octet sur le corpus |
| O3 | Robustesse | Aucune `panic` sur entrée arbitraire (fuzzing continu) |
| O4 | Diagnostic | Toute erreur localisée (record, champ, offset) ; ouverture partielle des fichiers abîmés |
| O5 | Adoption | Publication sur crates.io, documentation complète sur docs.rs |

## 3. Utilisateurs

| Utilisateur | Besoin |
|-------------|--------|
| NIST Studio (Rust) | Lecture, édition, écriture, diagnostic, accès aux données image |
| Outils CLI / scripts | Validation, transformation en lot, extraction |
| Services (serveur, pipeline) | Parsing rapide, sans état, sans I/O imposée |
| Développeurs tiers | API documentée, stable, licence libre utilisable depuis du code propriétaire (MIT) |

---

## 4. Périmètre fonctionnel

### 4.1 Parité avec la référence

| ID | Capacité | Attendu en Rust | Origine | Prio. | Statut |
|----|----------|-----------------|---------|-------|--------|
| LIB-01 | Parsing d'un message : découpage en records via le CNT (1.003) | Lecture depuis `&[u8]`, `Read` ou chemin | REF | M | Livré |
| LIB-02 | Records tagués `T.NNN:` (Type-1, 2, 9, 10, 13–21, 98, 99) | Idem | REF | M | Livré |
| LIB-03 | Records binaires à en-tête fixe (Type-3 à 8), `pack`/`unpack` de l'en-tête | En-têtes typés par type de record | REF | M | Livré |
| LIB-04 | Hiérarchie champ → sous-champ → item, séparateurs FS `0x1C` / GS `0x1D` / RS `0x1E` / US `0x1F` | Modèle typé `Message` → `Record` → `Field` → `Subfield` → `Item` | REF | M | Livré |
| LIB-05 | Champs binaires (données image du champ 999, données des Type-3 à 8) | Accès aux octets sans recopie | REF | M | Livré |
| LIB-06 | Accès par position, par clé `(type, IDC)`, itération par type | API idiomatique (itérateurs) | REF | M | Livré |
| LIB-07 | Alias de champs (`TOT`, `DAT`, `IDC`, `SLC`, `CGA`, `FGP`, `IMP`…) | Constantes / accesseurs nommés | REF | M | Livré |
| LIB-08 | Création d'un message avec Type-1 auto-créé et valeurs par défaut | Constructeur / builder | REF | M | Livré |
| LIB-09 | Ajout / suppression de records et de champs ; Type-1 unique et non supprimable | Idem, garanti par le typage quand possible | REF | M | Livré |
| LIB-10 | Génération : recalcul de LEN, du CNT (1.003), des IDC | Idem | REF | M | Livré |
| LIB-11 | Erreurs typées (BAD_RECORD, BAD_TAG_NAME, BAD_TAG_FORMAT, BAD_RECORD_NUMBER, BAD_CONTENT, CANNOT_ADD/DELETE_TYPE1, RECORD_NOT_FOUND, RECORD_NOT_TERMINATED, NIST_TOO_SHORT, NIST_TOO_LONG, BAD_TAG_DUPLICATE, BAD_ALIAS_DUPLICATE, UNKNOWN_ATTRIBUTE, BAD_FIELD/SUBFIELD_VALUE) | `enum Error` équivalent, avec position (record, champ, offset) | REF | M | Livré |

### 4.2 Améliorations

| ID | Exigence | Origine | Prio. | Statut |
|----|----------|---------|-------|--------|
| LIB-12 | **Écriture fidèle** : un fichier relu puis réécrit sans modification est identique octet pour octet (ordre des champs conservé ; la référence re-trie et normalise) | NEW | M | Livré (`Message::to_bytes_exact`) |
| LIB-13 | **Mode tolérant** : ouverture partielle des fichiers tronqués / mal formés + liste de diagnostics (erreurs et avertissements) | NEW | M | Livré (`ParseOptions::tolerant`, `Parsed::diagnostics`) |
| LIB-14 | Mode strict (comportement la référence) sélectionnable | NEW | M | Livré (`Message::parse`, comportement par défaut) |
| LIB-15 | Dictionnaire des champs par type : numéro → mnémonique, nom, format, cardinalité | NEW | S | Livré (feature `dictionary`, ANSI/NIST-ITL 1-2011 Update 2015) |
| LIB-16 | Validation selon la norme : champs obligatoires, formats, cardinalités, cohérence CNT/IDC | NEW | S | Livré (feature `validation`) |
| LIB-17 | Profils de validation : EBTS 11.2 (FBI) et INT-I 5.03 (INTERPOL) livrés ; les profils nationaux ne sont pas planifiés (l'énumération `Profile` est fermée : un nouveau profil s'ajoute dans la crate) | NEW | S | Partiel : EBTS 11.2 et INT-I 5.03 (`Message::validate_profile`) ; profils nationaux non planifiés (décision du 2026-11-10) |
| LIB-18 | Encodages déclarés en 1.015 (DCS) : ASCII, Latin-1, UTF-8, UTF-16 et UTF-32 livrés ; les séquences `STX` … `ETX` de l'annexe B.1.8 ne sont pas supportées (texte contenant un `STX` refusé) | NEW | S | Partiel : ASCII, Latin-1, UTF-8, UTF-16 et UTF-32 (module `encoding` ; UTF-16/32 : BOM reconnu, big-endian par défaut et à l'écriture, caractère dont les octets contiennent `0x1C` à `0x1F` refusé à l'encodage ; issue #85) ; `STX` non supporté |
| LIB-19 | Formats « traditionnels » (versions 0200 à 0500) : lus, réécrits et écrits à l'identique, sans règle de validation par version | NEW | S | Livré (`tests/versions.rs`) |
| LIB-20 | Accès typé aux métadonnées image communes (résolution, compression, dimensions, position doigt/paume) — **sans décodage** | NEW | M | Livré (module `image`) |
| LIB-21 | Libellés lisibles des codes (positions FGP, types d'impression IMP, compressions CGA) | NEW | S | Livré (module `codes`) |
| LIB-22 | Représentation XML ANSI/NIST-ITL (NIEM) | NEW | C | Non livré en 1.x ; envisagé pour une 2.x, sous condition (décision du 2026-11-10) |
| LIB-23 | Sérialisation `serde` optionnelle (JSON pour débogage / interop) | NEW | C | Livré (feature `serde`) |

### 4.3 Hors périmètre

| Sujet | Raison |
|-------|--------|
| Décodage / encodage d'images (WSQ, JPEG 2000…) | Responsabilité d'une autre crate (pour WSQ : `wsq-codec`, voir ci-dessous) ; la bibliothèque expose les octets bruts et les métadonnées |
| Interface graphique | Bibliothèque pure |
| Matching biométrique, extraction de minuties | Hors sujet |
| Transport réseau (envoi à un AFIS) | Hors sujet |

**WSQ.** La crate sœur existe : `wsq-codec` (décodeur et encodeur WSQ en Rust, crate publique distincte ; le nom `wsq` est déjà pris sur crates.io par une crate WebSockets). `ansi-nist-itl` n'en dépend pas : le README montre comment une application décode le champ 999 d'un Type-14 avec `wsq-codec`. Une feature optionnelle `wsq`, qui ferait le pont directement dans cette crate, est suivie par l'issue #75, conditionnelle à la sortie de `wsq-codec` 1.0.

---

## 5. Principes d'API

1. **Aucune I/O imposée** : le cœur travaille sur des octets ; la lecture de fichier est un confort.
2. **Zéro-copie en lecture** quand c'est possible (emprunts sur le buffer), modèle possédé pour l'édition — arbitrage tranché : modèle possédé unique, octets binaires partagés sans recopie via `Bytes` (LIB-05).
3. **Erreurs riches** : chaque erreur porte sa position ; jamais de `panic`, jamais d'`unwrap` sur des données d'entrée.
4. **Typage** plutôt que conventions (types de records, positions, compressions en `enum`).
5. **Pas d'`unsafe`** (`#![forbid(unsafe_code)]`), sauf justification documentée.
6. **Dépendances minimales** ; fonctionnalités optionnelles derrière des *features* Cargo (livrées : `serde`, `dictionary`, `validation`, `local-date` ; `xml` n'est pas livrée, voir LIB-22).
7. **Documentation** : chaque élément public documenté avec exemple exécutable.

## 6. Qualité et tests

| ID | Exigence | Prio. | Statut |
|----|----------|-------|--------|
| QA-01 | Portage des doctests de la référence comme tests unitaires (sorties binaires attendues) | M | Livré (`tests/reference_doctests.rs`) |
| QA-02 | Test d'oracle : sur le corpus, la structure parsée est identique à celle de la référence (types, IDC, champs, valeurs) | M | Livré (`tests/oracle.rs`, `tools/oracle/`) |
| QA-03 | Aller-retour lecture → écriture → lecture sans perte sur tout le corpus | M | Livré (oracle et `tests/properties.rs`) |
| QA-04 | Fuzzing du parseur (`cargo-fuzz`) en CI | M | Livré (3 cibles `cargo-fuzz`, exécution hebdomadaire planifiée : `.github/workflows/scheduled.yml`) |
| QA-05 | Tests de propriétés (générer → écrire → relire = identité) | S | Livré (`tests/properties.rs`) |
| QA-06 | CI : `cargo test`, `clippy -D warnings`, `rustfmt`, `cargo doc`, Linux / macOS / Windows | M | Partiel : Linux seulement (workflow partagé `ybdn-ei/.github`, `ubuntu-latest`) ; macOS et Windows non testés en CI |
| QA-07 | MSRV (version minimale de Rust) déclarée et testée | S | Livré (MSRV 1.86 déclarée et testée en CI) |
| QA-08 | Benchmarks (`criterion`) sur des fichiers de taille réaliste | C | Livré (`benches/message.rs`, compilés en CI) |

### 6.1 Données de test — point critique

Les fichiers réels (corpus de la v1, `nist-files/`) contiennent des **données biométriques et personnelles** : ils ne doivent **jamais** être versionnés dans ce dépôt public.

| ID | Exigence | Prio. | Statut |
|----|----------|-------|--------|
| DATA-01 | Dépôt public : uniquement des fichiers **synthétiques** (générés par la crate ou issus d'exemples publics de la norme / du NIST) | M | Livré |
| DATA-02 | Images de test synthétiques ou libres de droits, sans donnée réelle | M | Livré |
| DATA-03 | Test d'oracle sur corpus réel (QA-02, QA-03) exécuté **hors dépôt public** (dépôt privé de tests ou `nist-studio-rust`), via un chemin configurable | M | Livré (`ANSI_NIST_ITL_CORPUS`, voir ci-dessous) |
| DATA-04 | Garde-fou : hook / CI refusant l'ajout de fichiers NIST non référencés dans une liste blanche | S | Livré (`tools/check-data-files.sh` et liste blanche `tests/data/ALLOWLIST`, job CI `data`) |

**Corpus réel et banc d'oracle.** Le corpus réel reste sur la machine du mainteneur et n'entre jamais dans le dépôt. Le banc d'oracle (`tests/oracle.rs`, avec `tools/oracle/dump.py` pour la référence Python) le lit à l'emplacement donné par la variable d'environnement `ANSI_NIST_ITL_CORPUS` ; la même variable permet de lancer le banc de benchmarks sur un corpus réel. L'issue #82 suit ce sujet. La CI exécute le banc d'oracle sur un corpus synthétique de 20 000 fichiers (`tools/oracle/gen_corpus.py`, graine = numéro d'exécution). Voir `tools/oracle/README.md`.

## 7. Licence et propriété intellectuelle

- **Licence** : **MIT** (décidé le 2026-11-02, remplace une décision antérieure ; confirmée le 2026-11-10, #78). Fichier `LICENSE` ; `license = "MIT"` dans `Cargo.toml`. `nistitl` (IDEMIA, CeCILL-C) est cité comme référence de compatibilité : oracle de test uniquement, ni distribué ni dépendance.
- **Nom** : pas de reprise du nom la référence.

- **Titulaire du copyright** : ybdn, à titre personnel.
- **Nom** : `ansi-nist-itl` libre sur crates.io au 2026-10-20 ; aucune crate ANSI/NIST-ITL existante (pas de concurrent direct).

## 8. Jalons proposés

| Version | Contenu |
|---------|---------|
| 0.1 | Lecture des records tagués et binaires, modèle de données, erreurs typées (LIB-01 à 07, LIB-11) |
| 0.2 | Écriture, recalculs LEN/CNT/IDC, création de message (LIB-08 à 10), aller-retour fidèle (LIB-12) |
| 0.3 | Mode tolérant + diagnostics (LIB-13, LIB-14), fuzzing en CI |
| 0.4 | Métadonnées image, libellés, encodages (LIB-18, LIB-20, LIB-21) |
| 0.5 | Dictionnaire de champs + validation de base (LIB-15, LIB-16) |
| 1.0 | API stabilisée, parité la référence validée par oracle, documentation complète, publication crates.io |

État : jalons atteints. Les versions publiées regroupent les jalons proposés : 0.1.0 (parité avec la référence), 0.2.0 (dictionnaire, validation, profils, encodages, métadonnées image, `serde`), 1.0.0 (2026-10-23, API stable), 1.0.1 (2026-11-02, correctif de validation du Type-9). Voir `CHANGELOG.md`.

## 9. Questions ouvertes — closes au 2026-11-10

1. ~~Versions de norme à couvrir en 1.0~~ (décidé : les tables du dictionnaire et de la validation sont celles d'ANSI/NIST-ITL 1-2011 Update 2015, versions 0500 à 0502 ; les versions traditionnelles 0200 à 0500 sont lues et écrites sans règle de validation par version, LIB-19 ; les éditions postérieures ne sont pas couvertes)
2. ~~Profils de validation prioritaires~~ (décidé le 2026-11-10 : EBTS 11.2 et INT-I 5.03 livrés ; profils nationaux non planifiés, LIB-17)
3. ~~Format XML (NIEM)~~ (décidé le 2026-11-10 : non livré en 1.x, envisagé pour une 2.x sous condition, LIB-22)
4. ~~Modèle mémoire~~ (décidé : modèle possédé unique, octets binaires partagés sans recopie via `Bytes`, voir §5, principe 2)
5. ~~Titulaire du copyright~~ (décidé : ybdn, à titre personnel) — compte de publication crates.io à créer / lier au GitHub perso.
6. ~~Crate WSQ séparée et publique ?~~ (décidé : oui, `wsq-codec`, crate distincte ; feature optionnelle `wsq` suivie par l'issue #75, conditionnelle à `wsq-codec` 1.0, voir §4.3)
7. ~~Langue de la documentation publique~~ (décidé : anglais pour le README, la documentation rustdoc, le CHANGELOG et les commentaires du code ; ce PRD reste en français)
8. ~~Encodages de 1.015~~ (décidé le 2026-11-10 : ASCII, Latin-1, UTF-8, UTF-16 et UTF-32 livrés, issue #85 ; `STX` non supporté, LIB-18)
