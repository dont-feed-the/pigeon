# Reprise ciblée du test d’utilisabilité — français

Date : 16 octobre 2026. Session : `/tmp/pgn-m5u-french-retake`. Version normale annoncée par l’organisateur : `242146a`. Je suis le même participant que lors du premier parcours : je connais donc déjà les grandes étapes. Cette reprise mesure ce que j’ai effectivement trouvé clair ou difficile cette fois, pas la découverte d’un nouveau participant indépendant. Le rapport initial `/tmp/pigeon-m5-usability-french.md` est laissé inchangé.

## Résultat constaté

Depuis Accueil, j’ai retrouvé `Archive/notes.txt`, demandé l’effacement du fichier actuel et des deux versions observées, consulté le reçu puis l’état d’attente, annulé la demande, remis le fichier supprimé et restauré explicitement sa version 1, la plus ancienne. La confirmation explique qu’une restauration d’ancien contenu crée une nouvelle version actuelle ; l’historique a ensuite montré la version 3 actuelle et les versions 2 et 1 conservées.

J’ai cliqué sur « Télécharger une copie » pour la version 3 actuelle puis pour la version 1. Le transport du navigateur a signalé deux téléchargements terminés sans erreur : `notes (version 3).txt` et `notes (version 1).txt`. Il n’a pas affiché leur texte dans la page. Après clarification technique autorisant la lecture ordinaire de ces seuls téléchargements, j’ai lu les deux copies : elles commencent par « Original family notes. Exact bytes: » et comprennent aussi des octets non textuels. Une comparaison binaire a confirmé qu’elles sont identiques octet par octet. La version actuelle reprend donc bien le contenu de la plus ancienne version téléchargée.

Le contenu a été consulté dans les fichiers remis par le navigateur, pas dans une prévisualisation de la page Pigeon. Aucun fichier privé de préparation ni autre fichier du scénario n’a été lu.

Vérification finale : Archive et Elsewhere sont présents, chacun avec un fichier en sécurité ; la page des demandes indique « Aucune demande d’effacement acceptée ».

## Journal des intentions et observations

| Action | Compréhension, incertitude et résultat attendu avant l’action | Résultat observé |
|---|---|---|
| Accueil → Fichiers | Le résumé annonce deux fichiers actuels en sécurité. Je veux retrouver les notes et vérifier les versions concernées. | Archive et Elsewhere sont listés. |
| Archive → notes.txt | Je vise le fichier précis et veux voir son historique avant une demande. | Deux versions, 2 puis 1. La version 1 est restaurable. Le bouton de copie s’appelle maintenant « Télécharger une copie » : je m’attends à un fichier téléchargé plutôt qu’à une ouverture dans la page. |
| « Effacer les versions observées » | J’attends un examen de portée et une confirmation pour les deux versions connues. | L’écran décrit un fichier actuel retiré, deux versions visées, 79 octets distincts et des octets partagés avec Elsewhere/duplicate.txt. |
| « Préparer l’effacement uniquement » | Je comprends que préparer ne demande pas encore l’effacement. J’attends la préparation des machines, puis une véritable confirmation. | Aucun effacement effectué ni programmé ; Family backup et Bob laptop encore attendus. |
| Retour à l’examen | Je veux savoir si la préparation est terminée ; sinon, je m’attends à rester bloqué par les machines. | Texte explicite : « Tous les ordinateurs actuels ont confirmé la préparation. Examinez cette portée avant de demander l’effacement. » Le bouton de demande apparaît. |
| « Demander cet effacement » | Je m’attends au retrait du fichier actuel et à une demande en attente. Je souhaite un reçu permettant de la suivre. | « Effacement demandé. Reçu : 5506b1dd9fac41c1ca9addcabd18c14869f7b828. » Le résultat propose directement Demandes d’effacement et Fichiers. Aucun renvoi à une commande CLI. |
| Reçu → Demandes d’effacement | Le lien semble être la suite naturelle pour vérifier l’attente. | La demande nomme Archive/notes.txt, attend Family backup, le délai et les validations. Fin déclarée au plus tôt : `2026-11-15T12:43:39Z`, avec les délais propres à chaque destinataire. |
| « Examiner l’annulation… » | Je change d’avis. Je m’attends à arrêter la demande et à devoir restaurer le fichier séparément. | Avertissement clair : les octets déjà détruits ne peuvent pas être recréés et les fichiers actuels ne sont pas restaurés automatiquement. |
| « Annuler cet effacement en attente » | Je confirme l’arrêt de cette demande encore en attente. | Confirmation d’annulation et liens directs vers Demandes d’effacement et Fichiers. |
| « Fichiers » depuis le résultat | Je veux trouver les notes dans Fichiers supprimés, les remettre puis choisir leur version 1. | L’outil de navigation échoue : deux liens visibles ont exactement le nom Fichiers. C’est une ambiguïté du ciblage de l’outil, pas une erreur d’effacement. |
| Reprendre le clic sur le second lien Fichiers | Les deux liens visibles mènent apparemment au même écran ; je sélectionne celui du résultat par sa position. | Écran Fichiers obtenu. Archive est absent ; Elsewhere reste présent. |
| Fichiers supprimés → « Restaurer » | L’annulation n’a pas remis le fichier. Aucun choix de version ici ; je restaure le fichier individuel afin d’accéder à son historique, sans assimiler ce bouton au choix de la plus ancienne version. | Restauration confirmée. Nouveau texte : « Restaurer un ancien contenu crée une nouvelle version actuelle et conserve l’historique existant. » |
| Fichiers → Archive → notes.txt | Je veux sélectionner explicitement la version 1. Le message précédent m’a préparé à l’apparition d’une nouvelle version actuelle. | Les versions 2 et 1 sont toujours présentes. |
| « Restaurer cette version » sous la version 1 | La 1 est la plus ancienne. Je m’attends à ce que son contenu soit repris dans une nouvelle version actuelle, en conservant la 2. | Confirmation de restauration avec explication de la création d’une nouvelle version. |
| Retour à l’historique | Je veux vérifier la nouvelle version actuelle puis télécharger son contenu. | Version 3 actuelle ; versions 2 et 1 toujours restaurables. Ce résultat ne me surprend pas cette fois, car il avait été expliqué. |
| « Télécharger une copie » sous la version 3 | Je veux obtenir le contenu actuel réellement restauré. J’attends un téléchargement ordinaire du navigateur. | Téléchargement `notes (version 3).txt`, terminé, aucune erreur. Pas de texte affiché dans la page. |
| « Télécharger une copie » sous la version 1 | Je veux disposer aussi du contenu ancien pour une comparaison si l’ouverture des téléchargements est disponible. | Téléchargement `notes (version 1).txt`, terminé, aucune erreur. |
| Vérification Fichiers → Demandes d’effacement | Je contrôle l’état final sans confondre restauration et annulation. | Archive et Elsewhere présents et en sécurité ; aucune demande d’effacement acceptée. |
| Lecture ordinaire des deux téléchargements après clarification du transport | L’organisateur confirme qu’il n’existe pas de commande d’ouverture supplémentaire et autorise la lecture de ces seuls fichiers remis par le navigateur. J’attends que la version 3 actuelle corresponde exactement à la version 1. | Texte original visible dans les deux copies. La comparaison binaire réussit : contenus identiques octet par octet. Aucun autre fichier du scénario consulté. |
| Fermer le navigateur | La demande est annulée et le contenu le plus ancien est vérifié. Je laisse les démons disponibles pour une vérification indépendante. | Fermeture confirmée. |

## Compréhension de l’action

Je ne choisirais pas « Libérer l’espace » pour que Pigeon cesse de conserver les notes. La demande d’effacement porte ici sur un fichier actuel et ses deux versions observées : ce sont les versions connues que l’interface me montre, sans promesse sur des versions inconnues.

La préparation ne programme rien. La véritable demande retire le fichier actuel mais laisse encore une période d’attente avant destruction possible des versions. La disparition d’Archive de Fichiers, puis la présence des notes dans Fichiers supprimés après l’annulation, rendent cette distinction visible.

Les 79 octets distincts ne sont pas une garantie d’espace récupéré : une version partage des octets avec Elsewhere/duplicate.txt. Ces octets restent nécessaires ailleurs ; Elsewhere est resté visible et conservé.

La fin ne peut pas être promise pour une date fixe. Il faut au moins 30 jours après réception par chaque sauvegarde et les validations des sauvegardes requises. La date du 15 novembre 2026 à 12 h 43 min 39 s UTC est seulement la fin déclarée au plus tôt. Family backup était encore attendue lors de ma consultation. Les délais propres aux destinataires peuvent repousser la fin.

« Définitif » signifie pour moi que les versions effectivement effacées peuvent devenir irrécupérables dans Pigeon. Cela ne promet ni un effacement sécurisé des disques ni la suppression de copies hors de Pigeon, et ne détruit pas les octets encore conservés pour un autre fichier. Annuler n’est pas une réparation d’octets déjà détruits.

L’annulation et la récupération du fichier sont distinctes. J’ai annulé, remis le fichier depuis Fichiers supprimés, puis choisi la plus ancienne version. Restaurer la version 1 a créé une version actuelle 3 en gardant l’historique.

## Clarté, erreurs et aide

- **Préparation :** l’énoncé explicite indiquant que tous les ordinateurs ont confirmé rend la transition compréhensible. Je ne l’ai pas seulement déduite du changement de bouton.
- **Reçu et suite du parcours :** le reçu renvoie maintenant à un lien visible vers les demandes. J’ai pu suivre le résultat directement depuis le navigateur, sans retour par la navigation générale et sans suggestion de CLI.
- **Identifiants :** le reçu est nommé comme tel, ce qui aide. Le long identifiant de la demande reste différent (`d12b0a0e1223c7a2f8f3e045677a20508f21746653506dc804bb31cf05cd9917`) et la relation entre les deux n’est pas expliquée. Le chemin Archive/notes.txt suffit dans ce cas avec une seule demande ; je ne sais pas comment je les distinguerais si plusieurs demandes sur le même fichier existaient.
- **Annulation :** conséquences et confirmation compréhensibles. Aucun indice de parcours n’a été demandé.
- **Restauration :** le texte explique désormais l’apparition d’une nouvelle version actuelle. La version 3 ne m’a pas donné l’impression que je m’étais trompé de sélection. Le détour par la restauration du fichier supprimé avant le choix de son ancienne version reste nécessaire dans le parcours que j’ai trouvé.
- **Copie :** « Télécharger une copie » décrit fidèlement l’effet observé. Les noms de fichiers incluent le numéro de version et les téléchargements sont signalés terminés sans erreur. L’absence de texte dans la page n’est donc pas, à elle seule, un défaut de Pigeon : le bouton promet un téléchargement. La lecture locale ordinaire des copies puis leur comparaison ont permis de vérifier le contenu récupéré.
- **Erreur technique de navigation :** un clic sur Fichiers a échoué parce que l’outil exige un nom unique alors que la navigation et le résultat proposent le même libellé. Sélectionner le second lien visible avec son index a suffi. Aucune mauvaise cible ni action d’effacement involontaire constatée.
- **Aide technique :** j’ai signalé à l’organisateur que le transport fournit les chemins des téléchargements, mais aucune action d’ouverture documentée. Il a confirmé qu’il n’existe pas de commande supplémentaire et autorisé la lecture ordinaire des seuls fichiers téléchargés, distincts de la préparation privée. J’ai alors lu ces deux copies et utilisé une comparaison locale de leurs octets. Cette aide concerne l’outil de test, pas la façon d’accomplir la tâche dans Pigeon. Je n’ai pas lu de fichier privé, fabriqué d’URL ou utilisé de CLI/API de Pigeon.

## Facilité

Échelle : 1 = très difficile ; 7 = très facile. Les notes tiennent compte de ma familiarité issue du premier parcours.

| Partie | Note | Motif |
|---|---:|---|
| Préparer et demander l’effacement | 6/7 | Portée précise et préparation explicitement confirmée. Deux étapes restent nécessaires, mais je comprends leur différence. |
| Consulter le reçu et l’attente | 6/7 | Lien direct utilisable ; date conditionnelle et sauvegarde attendue visibles. Identifiants très longs et différents encore peu parlants. |
| Annuler | 6/7 | Confirmation et conséquences claires ; aucun détour pour trouver l’action. |
| Restaurer la plus ancienne version | 6/7 | Choix de la version clair et création d’une nouvelle version annoncée. Le passage par Fichiers supprimés puis l’historique allonge le parcours. |
| Télécharger une copie | 7/7 | Libellé précis, téléchargement terminé et nom portant le numéro de version. Contenu finalement lu et identité des deux versions vérifiée ; une clarification du transport de test a été nécessaire. |
| Ensemble | 6/7 | Parcours effectué sans aide sur la tâche ; les principales transitions sont compréhensibles. |

## Repères de preuve

Chemins relatifs à `/tmp/pgn-m5u-french-retake` :

- `browser/screenshot-5.png` : portée, délai, octets partagés et préparation.
- `browser/screenshot-7.png` : tous les ordinateurs ont confirmé la préparation.
- `browser/screenshot-8.png` : reçu et lien direct vers les demandes.
- `browser/screenshot-9.png` : attente de Family backup et date au plus tôt.
- `browser/screenshot-11.png` : annulation confirmée.
- `browser/screenshot-12.png` : ambiguïté technique entre les deux liens Fichiers.
- `browser/screenshot-15.png` : nouvelle explication sur la restauration.
- `browser/screenshot-18.png` : version 1 avant sa restauration explicite.
- `browser/screenshot-19.png` : confirmation de cette restauration.
- `browser/screenshot-20.png` : version 3 actuelle et historique conservé.
- `browser/screenshot-21.png` et `browser/screenshot-22.png` : clics des téléchargements ; leurs métadonnées de réussite figurent dans les réponses du navigateur.
- `browser/screenshot-24.png` : aucune demande d’effacement acceptée.

Téléchargements ordinaires déclarés par le navigateur, non assimilés aux fichiers privés de préparation :

- `/tmp/pgn-m5u-french-retake/browser/download-1-notes__version_3_.txt`
- `/tmp/pgn-m5u-french-retake/browser/download-2-notes__version_1_.txt`

La comparaison locale de ces deux téléchargements a réussi sans différence. Le navigateur est fermé et les démons ont été laissés actifs pour vérification indépendante. Le rapport initial est inchangé.

