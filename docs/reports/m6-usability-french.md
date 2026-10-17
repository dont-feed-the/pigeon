# Session d’utilisation indépendante — interface française M6

Date : 17 octobre 2026. Session effectuée par un agent jouant le rôle d’une personne francophone qui n’a pas construit pigeon ; ce n’est pas une étude avec un participant humain. Périmètre : HTTP et texte, pages et formulaires réellement servis par le démon isolé `/tmp/pigeon-m6-usability-fr`. Aucun code de l’application n’a été consulté ou modifié. Ni étude visuelle, ni mesure de débit, ni essai matériel de batterie. Aucun groupe n’était rejoint ; l’accueil affichait « Cet ordinateur n'est pas membre du groupe. ».

Connexion : lien à usage unique obtenu avec `pigeon --json open`, consommé avec une session HTTP et un cookie. Navigation par les liens et formulaires HTML ; JavaScript non exécuté. Les confirmations ci-dessous concernent donc le parcours HTTP sans JavaScript, pas les notifications affichées par le navigateur avec JavaScript.

## Résultats par tâche

1. **Garder le partage discret sur un portable : réussi pour le choix du réglage.** Le lien « Réglages » était directement présent dans la navigation. La section « Utilisation de cet ordinateur » disait « Discret laisse le partage en arrière-plan. » Le préréglage initial était déjà « Discret » ; je l’ai enregistré. Confirmation : « Préréglage : Discret. » C’est une indication compréhensible de l’intention ; la discrétion matérielle réelle n’a pas été évaluée.

2. **Limiter l’envoi à 2 MB/s : réussi.** L’intitulé « Envoi maximal (auto ou 2 MB/s) » donne précisément l’exemple nécessaire. J’ai saisi `2 MB/s` et enregistré. Le retour aux réglages montrait toujours `2 MB/s`. En revanche, la page de confirmation affichait littéralement « Envoi maximal (auto ou 2 MB/s) : setting-2 MB/s. » Le préfixe `setting-` ressemble à un identifiant interne et dégrade une confirmation autrement simple.

3. **Changer de préréglage et comprendre la priorité des réglages manuels : réussi.** J’ai choisi « Rapide ». Confirmation : « Préréglage : Rapide. » Au retour, « Rapide » était sélectionné et le champ d’envoi conservait `2 MB/s`. L’explication visible « Vos réglages détaillés remplacent le préréglage. » me permet de comprendre qu’un plafond manuel conserve sa priorité. Les valeurs par défaut non modifiées suivent le préréglage : après Rapide, le seuil de batterie affichait `0` (il affichait `20` au départ), et le réseau à consommation limitée affichait « Continuer ».

4. **Configurer 22:00–23:00 à pleine vitesse : enregistrement réussi, sens du plafond non résolu dans l’interface.** La phrase « séparez les plages HH:MM-HH:MM par des virgules » rend le format découvrable. `22:00-23:00` a été accepté et conservé. Confirmation littérale : « Plages à pleine vitesse (vide : aucune) : setting-22:00-23:00. » Aucune phrase sur cette page n’expliquait si « pleine vitesse » lève le plafond manuel d’envoi. Comme francophone, je peux lire « Envoi maximal » comme une limite absolue, mais « pleine vitesse » suggère aussi une exception. Je ne peux pas répondre avec certitude à la question du plafond à partir des pages testées. La persistance simultanée des deux réglages ne prouve pas leur priorité d’exécution.

5. **Saisir un débit invalide puis récupérer : réussi pour l’intégrité et la récupération, aide faible.** J’ai saisi `banane` dans l’envoi maximal. La réponse était une page d’erreur, avec HTTP 200, contenant exactement : « « banane » n'est pas une valeur possible de « resources.upload ». », « Voyez les valeurs avec : pigeon settings », « Code : P-SETTING-VALUE », puis « Retour ». Le nom technique `resources.upload` remplace l’intitulé français et l’aide demande une commande au lieu de rappeler un exemple valide. Aucun champ de correction n’était présent sur cette page. Après « Retour » vers Réglages, le plafond précédent `2 MB/s`, le préréglage Rapide et la plage `22:00-23:00` étaient conservés. J’ai réenregistré `2 MB/s` avec succès.

6. **Choisir un seuil de batterie et distinguer attente d’alimentation et pause humaine : réglages et pause humaine réussis, distinction opérationnelle partiellement expliquée.** J’ai choisi `30` pour « Seuil de batterie faible (%) » et « Attendre » pour « Quand la batterie est faible ». Les valeurs sont restées affichées. Le seuil a produit « Seuil de batterie faible (%) : setting-30. » L’intitulé « Quand la batterie est faible » et la paire « Attendre / Continuer » indiquent une politique conditionnelle ; la section distincte « Partage » offre une pause volontaire de durée choisie. J’ai réellement demandé une pause d’une heure : « Le partage est en pause jusqu’à 09 h 35. Les changements ne sont enregistrés que sur cet ordinateur. » Le bouton « Reprendre le partage » a ensuite produit « Le partage reprend. » La politique batterie et le seuil de 30 sont restés enregistrés après la reprise. Cependant, le texte de Réglages ne précise pas que Reprendre peut laisser une attente d’alimentation active, ni quand cette attente prend fin. Aucune batterie faible n’a été provoquée ; je ne peux pas établir le comportement matériel ou l’état d’attente réel.

## Constats prioritaires

- **Ambiguïté à résoudre :** préciser si le plafond manuel d’envoi continue à s’appliquer pendant une plage « à pleine vitesse ». C’est la seule tâche dont l’explication attendue n’a pas pu être trouvée dans le parcours.
- **Confirmation défectueuse en HTTP sans JavaScript :** les valeurs libres sont précédées de `setting-` (`setting-2 MB/s`, `setting-22:00-23:00`, `setting-30`). Les choix prédéfinis Discret, Rapide et Attendre sont correctement traduits.
- **Récupération peu guidée :** l’erreur de débit expose `resources.upload`, renvoie au CLI et retire le champ au profit d’une page « Retour ». Une correction reste possible et les valeurs précédentes sont protégées.
- **Distinction batterie/pause à rendre explicite :** les sections et verbes différents donnent un premier indice, mais aucune explication visible n’indique la priorité de la politique batterie lors d’une reprise manuelle. Le texte « Le partage reprend. » peut paraître absolu.
- **Points efficaces :** accès direct à Réglages ; exemple de débit exactement utilisable ; phrase explicite sur la priorité des réglages détaillés ; conservation de la limite manuelle après changement de préréglage ; conservation des réglages après rejet d’une valeur invalide.

## Vérification indépendante après les tâches

Les commandes et le fichier de configuration n’ont été consultés qu’une fois les tâches terminées.

`PIGEON_SOCKET=/tmp/pigeon-m6-usability-fr/run/pigeond.sock /workspace/pigeon-target-m6/debug/pigeon --json settings` confirmait :

| Clé | Valeur finale |
| --- | --- |
| language | fr |
| resources.preset | fast |
| resources.upload | 2 MB/s |
| resources.download | auto |
| resources.on_battery | pause |
| resources.battery_percent | 30 |
| resources.on_metered | continue |
| resources.full_speed | 22:00-23:00 |
| resources.scrub_window | 01:00-06:00 |

Le fichier réel `/tmp/pigeon-m6-usability-fr/config/config.toml` contenait dans `[resources]` : `preset = "fast"`, `upload = "2 MB/s"`, `full_speed = ["22:00-23:00"]`, `battery_percent = 30`, `on_battery = "pause"`. Cela confirme la persistance, pas les effets de débit ou d’alimentation.

`pigeon --json status` renvoyait le problème `not-in-group`. Aucune mesure de partage actif ne peut donc être déduite de cette session.

Note méthodologique : une première soumission automatisée avait omis le champ caché `form` après avoir réutilisé la page de confirmation au lieu de revenir aux réglages. Elle a reçu HTTP 422 « missing field `form` ». C’était une erreur du harnais HTTP, corrigée en suivant Retour ; elle n’est pas comptée comme échec d’utilisation du produit. Aucun autre échec de soumission valide n’a été rencontré.
