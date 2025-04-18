# csv_tools

Outils Rust pour l’analyse et la réparation de fichiers CSV volumineux et complexes.

## Présentation

Ce projet propose plusieurs utilitaires en ligne de commande pour :
- Extraire l’en-tête d’un fichier CSV
- Compter le nombre de lignes
- Analyser la structure (nombre de champs par ligne)
- Analyser les valeurs distinctes d’un champ
- Réparer la structure de fichiers CSV corrompus ou ambigus

Ces outils sont adaptés aux fichiers volumineux (plusieurs millions de lignes) et aux cas complexes (champs multi-lignes, encodages variés, séparateurs ambigus).

## Prérequis

- Rust (edition 2024)
- Compilation :  
  ```sh
  cd csv_tools
  cargo build --release
  ```

## Utilisation des binaires

Chaque outil est un binaire indépendant, à lancer avec `cargo run --bin <nom> -- <options>` ou via l’exécutable compilé.

### 1. `extract_header`
- **But** : Extraire l’en-tête du CSV et générer `ListeVariablesContrats.txt`
- **Options** :
  - `--file <chemin>` : chemin du fichier CSV
  - `--encoding <encodage>` : utf-8, windows-1252, etc.
  - `--delimiter <séparateur>` : `,` ou `;` ou `\t`
- **Exemple** :
  ```sh
  cargo run --bin extract_header -- --file ../Evenements_anon.csv --delimiter ','
  ```

### 2. `count_lines`
- **But** : Compter le nombre de lignes du fichier
- **Options** : idem
- **Exemple** :
  ```sh
  cargo run --bin count_lines -- --file ../Evenements_anon.csv --max 10000 --delimiter ','
  ```

### 3. `count_fields`
- **But** : Analyser la distribution du nombre de champs par ligne (parser CSV strict)
- **Options** :
  - `--file <chemin>`
  - `--encoding <encodage>`
  - `--delimiter <séparateur>`
  - `--max <N>` : nombre max de lignes à lire
- **Exemple** :
  ```sh
  cargo run --bin count_fields -- --file ../Evenements_anon.csv --delimiter ',' --max 1000
  ```

### 3bis. `count_fields_raw`
- **But** : Analyser la distribution du nombre de champs par ligne (analyse brute, sans parser strict) donne les meilleurs résultats (peut être TRES TRES long)
- **Options** :
  - `--file <chemin>`
  - `--encoding <encodage>`
  - `--delimiter <séparateur>`
  - `--max <N>`
  - `--decimal <séparateur>` (pour gérer le séparateur décimal ambigu)
- **Exemple** :
  ```sh
  cargo run --bin count_fields_raw -- --file ../Evenements_anon.csv --delimiter ',' --max 1000 --decimal ','
  ```

### 4. `analyze_field`
- **But** : Compter les valeurs distinctes d’un champ donné (par nom ou index)(crash sur fichier non-corrigé)
- **Options** :
  - `--file <chemin>`
  - `--encoding <encodage>`
  - `--delimiter <séparateur>`
  - `--field-name <nom>` ou `--field-index <idx>`
  - `--max <N>`
- **Exemple** :
  ```sh
  cargo run --bin analyze_field -- --file ../Evenements_anon.csv --field-name TYPE_EVENEMENT --max 1000 --delimiter ','
  ```

### 4bis. `analyze_field_raw`
- **But** : Compter les valeurs distinctes d’un champ donné (analyse brute)
- **Options** :
  - `--file <chemin>`
  - `--encoding <encodage>`
  - `--delimiter <séparateur>`
  - `--field-name <nom>` ou `--field-index <idx>`
  - `--max <N>`
- **Exemple** :
  ```sh
  cargo run --bin analyze_field_raw -- --file ../Evenements_anon.csv --field-index 2 --max 1000 --delimiter ','
  ```

### 5. `repair_csv`
- **But** : Corriger la structure du CSV en marquant les lignes incohérentes (nombre de champs différent du nombre attendu).
- **Fonctionnement** : 
  - Lit le fichier ligne par ligne (mode tolérant).
  - Les lignes avec le bon nombre de champs sont recopiées telles quelles.
  - Les lignes avec un nombre de champs différent sont marquées en début de ligne par `#BAD (N champs)` et conservées dans le fichier de sortie.
  - Permet d’identifier rapidement les lignes problématiques pour une correction manuelle ou un post-traitement.
- **À utiliser** : pour obtenir un CSV “propre” où toutes les lignes sont présentes, mais les lignes incorrectes sont signalées.
- **Exemple** :
  ```sh
  cargo run --bin repair_csv -- --file ../Evenements_anon.csv --delimiter ',' --output ../Evenements_anon_corrected.csv --expected-fields 93 --max 100000
  ```

### 5bis. `repair_csv_auto`
- **But** : Correction automatique de la structure du CSV en tentant de fusionner les champs éclatés.
- **Fonctionnement** :
  - Lit le fichier ligne par ligne (mode tolérant).
  - Les lignes avec le bon nombre de champs sont recopiées telles quelles.
  - Les lignes avec trop de champs sont “réparées” automatiquement : les champs en trop sont fusionnés dans le dernier champ attendu.
  - Les lignes avec trop peu de champs sont marquées comme irrécupérables (`#BAD (N champs)`).
  - Produit un CSV utilisable directement pour la plupart des traitements automatiques.
- **À utiliser** : pour obtenir un CSV “corrigé” automatiquement, prêt à être exploité, même si certaines lignes sont imparfaites.
- **Exemple** :
  ```sh
  cargo run --bin repair_csv_auto -- --file ../Evenements_anon.csv --delimiter ',' --output ../Evenements_anon_corrected_auto.csv --expected-fields 93 --max 100000
  ```

## 6. `hyper_csv_analyze`
- **But** : Effectuer en un seul passage sur le fichier CSV : extraction d’entête, comptage de lignes, distribution du nombre de champs, analyse de plusieurs champs, et réparation automatique.
- **Fonctionnement** :
  - Lit le fichier ligne par ligne (streaming, très efficace).
  - À chaque ligne : met à jour le compteur de lignes, la distribution du nombre de champs, la distribution des valeurs pour chaque champ à analyser, et écrit la version réparée de la ligne dans un fichier de sortie.
  - Écrit l’entête dans `ListeVariablesContrats.txt` à la première ligne.
  - Permet d’obtenir tous les résultats d’analyse et un CSV corrigé en une seule lecture du fichier.
- **À utiliser** : pour gagner du temps sur les très gros fichiers, éviter de relire plusieurs fois, et obtenir toutes les analyses et corrections en une seule commande.
- **Exemple** :
  ```sh
  cargo run --bin hyper_csv_analyze -- --file ../Evenements_anon.csv --delimiter ',' --expected-fields 93 --analyze-fields 2,5 --output ../Evenements_anon_hyper_corrected.csv --max 100000
  ```
  (Ici, on analyse les distributions du champ 2 et du champ 5, en plus de toutes les autres analyses.)

## Exemples d’utilisation

```sh
cargo run --bin extract_header -- --file ../Evenements_anon.csv --delimiter ','
cargo run --bin count_lines -- --file ../Evenements_anon.csv --max 10000 --delimiter ','
cargo run --bin count_fields -- --file ../Evenements_anon.csv --delimiter ',' --max 100000
cargo run --bin analyze_field -- --file ../Evenements_anon.csv --field-name "TYPE_EVENEMENT" --max 50000 --delimiter ','
cargo run --bin hyper_csv_analyze -- --file ../Evenements_anon.csv --delimiter ',' --expected-fields 93 --analyze-fields 2,5 --output ../Evenements_anon_hyper_corrected.csv --max 100000
```

## Conseils pour éviter les crashs

- Utiliser l’option `--max` pour limiter le nombre de lignes traitées lors des premiers tests

## Axes d’amélioration (TODO)

- [ ] Ajouter une option `--top N` pour n’afficher que les N valeurs les plus fréquentes (pour les distributions et les valeurs de champ)
- [ ] Ajouter une option `--output fichier.txt` pour écrire la distribution dans un fichier
- [ ] Ajouter une option `--min-count N` pour n’afficher que les valeurs apparaissant au moins N fois
- [ ] Ajouter des tests unitaires sur des cas limites

## Licence

Projet pédagogique – Université 2024
