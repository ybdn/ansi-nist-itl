# PRD — ansi-nist-itl

> **Statut** : brouillon v0.1 — base de travail à amender
> **Date** : 2026-10-20
> **Auteur** : ybdn
> **Dépôt et crate** : `ansi-nist-itl` — nom disponible sur crates.io (vérifié le 2026-10-20)
> **Objet** : définir le périmètre de la bibliothèque Rust de lecture/écriture ANSI/NIST-ITL, qui remplace la bibliothèque Python `nistitl` et sert de fondation à NIST Studio (Rust).

**Légende — colonne « Origine »**

| Code | Signification |
|------|---------------|
| **NISTITL** | Capacité présente dans `nistitl` v0.6 → parité attendue |
| **NEW** | Absente de `nistitl` → amélioration |
| **?** | À décider |

**Colonne « Prio. »** : `M` (Must, 1.0) / `S` (Should) / `C` (Could) / `W` (Won't).
Les blocs `> À compléter` marquent les endroits où tes attentes doivent être précisées.

---

## 1. Contexte

- La v1 de NIST Studio (Python) délègue tout le traitement des fichiers NIST à `nistitl` (IDEMIA, v0.6, ~1 900 lignes, licence CeCILL-C).
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
| O1 | Parité fonctionnelle avec `nistitl` | Toutes les exigences d'origine NISTITL couvertes ; doctests `nistitl` portés au vert |
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
| Développeurs tiers | API documentée, stable, licence libre utilisable depuis du code propriétaire (CeCILL-C) |

---

## 4. Périmètre fonctionnel

### 4.1 Parité avec `nistitl`

| ID | Capacité | Attendu en Rust | Origine | Prio. |
|----|----------|-----------------|---------|-------|
| LIB-01 | Parsing d'un message : découpage en records via le CNT (1.003) | Lecture depuis `&[u8]`, `Read` ou chemin | NISTITL | M |
| LIB-02 | Records tagués `T.NNN:` (Type-1, 2, 9, 10, 13–21, 98, 99) | Idem | NISTITL | M |
| LIB-03 | Records binaires à en-tête fixe (Type-3 à 8), `pack`/`unpack` de l'en-tête | En-têtes typés par type de record | NISTITL | M |
| LIB-04 | Hiérarchie champ → sous-champ → item, séparateurs FS `0x1C` / GS `0x1D` / RS `0x1E` / US `0x1F` | Modèle typé `Message` → `Record` → `Field` → `Subfield` → `Item` | NISTITL | M |
| LIB-05 | Champs binaires (données image du champ 999, données des Type-3 à 8) | Accès aux octets sans recopie | NISTITL | M |
| LIB-06 | Accès par position, par clé `(type, IDC)`, itération par type | API idiomatique (itérateurs) | NISTITL | M |
| LIB-07 | Alias de champs (`TOT`, `DAT`, `IDC`, `SLC`, `CGA`, `FGP`, `IMP`…) | Constantes / accesseurs nommés | NISTITL | M |
| LIB-08 | Création d'un message avec Type-1 auto-créé et valeurs par défaut | Constructeur / builder | NISTITL | M |
| LIB-09 | Ajout / suppression de records et de champs ; Type-1 unique et non supprimable | Idem, garanti par le typage quand possible | NISTITL | M |
| LIB-10 | Génération : recalcul de LEN, du CNT (1.003), des IDC | Idem | NISTITL | M |
| LIB-11 | Erreurs typées (BAD_RECORD, BAD_TAG_NAME, BAD_TAG_FORMAT, BAD_RECORD_NUMBER, BAD_CONTENT, CANNOT_ADD/DELETE_TYPE1, RECORD_NOT_FOUND, RECORD_NOT_TERMINATED, NIST_TOO_SHORT, NIST_TOO_LONG, BAD_TAG_DUPLICATE, BAD_ALIAS_DUPLICATE, UNKNOWN_ATTRIBUTE, BAD_FIELD/SUBFIELD_VALUE) | `enum Error` équivalent, avec position (record, champ, offset) | NISTITL | M |

### 4.2 Améliorations

| ID | Exigence | Origine | Prio. |
|----|----------|---------|-------|
| LIB-12 | **Écriture fidèle** : un fichier relu puis réécrit sans modification est identique octet pour octet (ordre des champs conservé ; `nistitl` re-trie et normalise) | NEW | M |
| LIB-13 | **Mode tolérant** : ouverture partielle des fichiers tronqués / mal formés + liste de diagnostics (erreurs et avertissements) | NEW | M |
| LIB-14 | Mode strict (comportement `nistitl`) sélectionnable | NEW | M |
| LIB-15 | Dictionnaire des champs par type : numéro → mnémonique, nom, format, cardinalité | NEW | S |
| LIB-16 | Validation selon la norme : champs obligatoires, formats, cardinalités, cohérence CNT/IDC | NEW | S |
| LIB-17 | Profils de validation (ITL 1-2011, mises à jour 2013/2015, EBTS FBI, Interpol, profils nationaux) | NEW | ? |
| LIB-18 | Encodages déclarés en 1.015 (DCS) : ASCII, Latin-1, UTF-8, UTF-16 | NEW | S |
| LIB-19 | Formats « traditionnels » (versions 0200 à 0500) | ? | S |
| LIB-20 | Accès typé aux métadonnées image communes (résolution, compression, dimensions, position doigt/paume) — **sans décodage** | NEW | M |
| LIB-21 | Libellés lisibles des codes (positions FGP, types d'impression IMP, compressions CGA) | NEW | S |
| LIB-22 | Représentation XML ANSI/NIST-ITL (NIEM) | NEW | ? |
| LIB-23 | Sérialisation `serde` optionnelle (JSON pour débogage / interop) | NEW | C |

### 4.3 Hors périmètre

| Sujet | Raison |
|-------|--------|
| Décodage / encodage d'images (WSQ, JPEG 2000…) | Responsabilité d'une autre crate (ex. côté NIST Studio, ou une future crate `wsq`) ; la bibliothèque expose les octets bruts et les métadonnées |
| Interface graphique | Bibliothèque pure |
| Matching biométrique, extraction de minuties | Hors sujet |
| Transport réseau (envoi à un AFIS) | Hors sujet |

> À compléter : faut-il prévoir une crate sœur pour WSQ (décodeur Rust pur), également publique ?

---

## 5. Principes d'API

1. **Aucune I/O imposée** : le cœur travaille sur des octets ; la lecture de fichier est un confort.
2. **Zéro-copie en lecture** quand c'est possible (emprunts sur le buffer), modèle possédé pour l'édition — arbitrage à faire en spécification.
3. **Erreurs riches** : chaque erreur porte sa position ; jamais de `panic`, jamais d'`unwrap` sur des données d'entrée.
4. **Typage** plutôt que conventions (types de records, positions, compressions en `enum`).
5. **Pas d'`unsafe`** (`#![forbid(unsafe_code)]`), sauf justification documentée.
6. **Dépendances minimales** ; fonctionnalités optionnelles derrière des *features* Cargo (`serde`, `xml`, `validation`).
7. **Documentation** : chaque élément public documenté avec exemple exécutable.

## 6. Qualité et tests

| ID | Exigence | Prio. |
|----|----------|-------|
| QA-01 | Portage des doctests de `nistitl` comme tests unitaires (sorties binaires attendues) | M |
| QA-02 | Test d'oracle : sur le corpus, la structure parsée est identique à celle de `nistitl` (types, IDC, champs, valeurs) | M |
| QA-03 | Aller-retour lecture → écriture → lecture sans perte sur tout le corpus | M |
| QA-04 | Fuzzing du parseur (`cargo-fuzz`) en CI | M |
| QA-05 | Tests de propriétés (générer → écrire → relire = identité) | S |
| QA-06 | CI : `cargo test`, `clippy -D warnings`, `rustfmt`, `cargo doc`, Linux / macOS / Windows | M |
| QA-07 | MSRV (version minimale de Rust) déclarée et testée | S |
| QA-08 | Benchmarks (`criterion`) sur des fichiers de taille réaliste | C |

### 6.1 Données de test — point critique

Les fichiers réels (corpus de la v1, `nist-files/`) contiennent des **données biométriques et personnelles** : ils ne doivent **jamais** être versionnés dans ce dépôt public.

| ID | Exigence | Prio. |
|----|----------|-------|
| DATA-01 | Dépôt public : uniquement des fichiers **synthétiques** (générés par la crate ou issus d'exemples publics de la norme / du NIST) | M |
| DATA-02 | Images de test synthétiques ou libres de droits, sans donnée réelle | M |
| DATA-03 | Test d'oracle sur corpus réel (QA-02, QA-03) exécuté **hors dépôt public** (dépôt privé de tests ou `nist-studio-rust`), via un chemin configurable | M |
| DATA-04 | Garde-fou : hook / CI refusant l'ajout de fichiers NIST non référencés dans une liste blanche | S |

> À compléter : où héberger le corpus réel et le banc d'oracle `nistitl` (Python) ?

## 7. Licence et propriété intellectuelle

- **Licence** : **CeCILL-C v1.0** (décidé le 2026-10-20, remplace la décision MIT du même jour). Fichiers `LICENSE` (version anglaise) et `LICENSE-fr` (version française), les deux faisant foi ; `license = "CECILL-C"` dans `Cargo.toml`.
- **Raison** : la spécification (`docs/SPEC.md`) reproduit le comportement de `nistitl` ligne à ligne ; la crate est donc un **portage** de `nistitl` (CeCILL-C, IDEMIA) et doit en suivre la licence.
- **Conséquences** : copyleft limité au composant. Les modifications de la crate sont redistribuées sous CeCILL-C ; un logiciel qui l'utilise comme dépendance (NIST Studio, propriétaire) reste libre de sa licence. Droit français, tribunaux de Paris (art. 13).
- **Crédits** : `nistitl` / IDEMIA cité dans le README comme œuvre d'origine.
- **Nom** : pas de reprise du nom `nistitl`.

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
| 1.0 | API stabilisée, parité `nistitl` validée par oracle, documentation complète, publication crates.io |

## 9. Questions ouvertes

1. Versions de norme à couvrir en 1.0 (1-2011 seule, ou aussi versions antérieures et mises à jour) ?
2. Profils de validation prioritaires (EBTS, Interpol, national) ?
3. Format XML (NIEM) : dans la 1.0, plus tard, ou jamais ?
4. ~~Modèle mémoire~~ (décidé : modèle possédé unique, octets binaires partagés sans recopie via `Bytes`, voir SPEC §14.6)
5. ~~Titulaire du copyright~~ (décidé : ybdn, à titre personnel) — compte de publication crates.io à créer / lier au GitHub perso.
6. Crate WSQ séparée et publique ? (le nom `wsq` est déjà pris sur crates.io par une crate WebSockets)
7. Langue de la documentation publique (EN recommandé pour l'adoption ; ce PRD reste en FR) ?
