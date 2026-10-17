# Test d’utilisabilité Pigeon — participant francophone

Date : 16 octobre 2026. Session isolée : `/tmp/pgn-m5u-french`. Compte : Nora, administratrice. Parcours commencé sur Accueil. Je n’ai utilisé que les liens et boutons de l’interface web pour agir sur Pigeon. Aucun code, document du dépôt, fichier privé de préparation, CLI ou API de Pigeon n’a été consulté. Aucune aide ni aucun indice n’a été demandé.

## Résultat

J’ai demandé l’effacement de `Archive/notes.txt` et des deux versions observées. J’ai ensuite annulé cette demande encore en attente, restauré le fichier supprimé, puis choisi explicitement la version 1, la plus ancienne. L’interface a confirmé cette restauration ; une version 3 actuelle est apparue et les versions 2 et 1 sont restées dans l’historique. Enfin, la page des demandes affichait « Aucune demande d’effacement acceptée ».

Je considère le parcours accompli d’après les confirmations de l’interface. Je n’ai pas pu comparer le contenu de la version actuelle avec celui de la version 1 : « Ouvrir une copie » n’a eu aucun effet visible dans le navigateur. Cette limite de vérification est distincte de la réussite du clic de restauration et de sa confirmation.

## Journal : compréhension avant les actions et résultats observés

| Étape et action | Ce que je comprenais, mes incertitudes et le résultat attendu avant l’action | Résultat observé |
|---|---|---|
| Accueil → Fichiers | Accueil sépare fichiers actuels et autres versions conservées. Je cherche les notes dans Fichiers ; je ne sais pas encore si supprimer le fichier supprime aussi son historique. | Archive et Elsewhere sont listés, chacun avec un fichier. Liens « Demandes d’effacement » et « Fichiers supprimés ». |
| Ouvrir Archive | « Libérer l’espace » me semble concerner le stockage local ; « Demandes d’effacement » semble mieux correspondre à une suppression durable. J’ouvre le dossier pour cibler le bon fichier. | Lien vers notes.txt et lien « Effacer les versions observées » au niveau du dossier. |
| Ouvrir notes.txt | Je préfère agir sur le fichier précis plutôt que sur tout le dossier. J’attends son historique. | Deux versions : 2, récente, et 1, plus ancienne ; la version 1 a un bouton « Restaurer cette version ». |
| « Effacer les versions observées » depuis le fichier | J’attends une explication ou une confirmation pour les deux versions connues. | L’écran annonce le retrait d’un fichier actuel, la demande d’effacement de deux versions, 79 octets distincts, le délai et les octets partagés avec Elsewhere/duplicate.txt. |
| « Préparer l’effacement uniquement » | Je comprends que cela prépare les machines et n’efface rien immédiatement. Je suis encore incertain du moment où le fichier actuel disparaîtra. | « Aucun effacement n’a été effectué ni programmé. » Il faudra examiner et confirmer à nouveau. Bob laptop et Family backup sont encore attendus. |
| Retour vers l’examen | J’espère retrouver la confirmation quand les machines seront prêtes. | Les mentions d’attente ont disparu et le bouton est devenu « Demander cet effacement ». Je n’ai eu à agir sur aucune autre machine. |
| « Demander cet effacement » | Je m’attends maintenant au retrait du fichier actuel et à une demande sur ses deux versions, encore annulable pendant l’attente. | Reçu « Effacement demandé » avec un identifiant et une référence à la commande `pigeon erase`. Pas de lien direct vers l’état de cette demande. |
| Fichiers → Demandes d’effacement | Le reçu CLI me laisse hésitant dans le navigateur. Je cherche la demande dans l’interface pour la vérifier et changer d’avis. | Archive a disparu ; Elsewhere reste. La demande nomme Archive/notes.txt, attend Family backup et le délai de conservation. |
| « Examiner l’annulation… » | Le chemin du fichier identifie la bonne demande malgré un long identifiant différent de celui du reçu. Je m’attends à une confirmation avant annulation. | L’écran explique que l’annulation ne reconstitue pas des octets déjà supprimés et ne restaure pas automatiquement les fichiers actuels. |
| « Annuler cet effacement en attente » | Je veux arrêter la demande. Comme le délai n’est pas écoulé, je m’attends à pouvoir restaurer ensuite le fichier séparément. | Confirmation explicite de l’annulation et rappel de restaurer séparément les fichiers souhaités. |
| Fichiers → Fichiers supprimés | Je m’attends à y retrouver notes.txt et peut-être son historique. | Archive et notes.txt figurent dans la page. Deux possibilités : restaurer le dossier ou restaurer le fichier. Aucun choix de version ici. |
| « Restaurer » pour le fichier | J’en déduis qu’il faut remettre le fichier d’abord, puis sélectionner la plus ancienne version dans son historique. Je m’attends à remettre son dernier état enregistré. | Le message confirme une restauration sur cet ordinateur et dit que « la version plus récente reste dans son historique ». Cette formulation me surprend puisque je n’ai pas encore choisi de version. |
| Fichiers → Archive → notes.txt | Je veux lever ce doute en choisissant explicitement la version la plus ancienne. | L’historique présente encore les versions 2 et 1, comme avant la demande. |
| « Restaurer cette version » sous la version 1 | La version 1 est la plus ancienne. Les dates relatives sont désormais presque identiques, mais les numéros et l’ordre me permettent de la choisir. J’attends qu’elle devienne le contenu actuel et que la version récente reste conservée. | Confirmation de restauration de notes.txt ; la version plus récente reste dans l’historique. |
| Retour vers l’historique | Je m’attends, à tort, à ce que la version 1 n’ait plus de bouton de restauration puisqu’elle serait actuelle. | Une version 3 actuelle est apparue. Les versions 2 et 1 restent restaurables. Je comprends alors que restaurer crée une nouvelle version actuelle. |
| « Ouvrir une copie » pour la version 3 | Je veux comparer le contenu actuel avec la version 1, car l’historique ne dit pas explicitement « restaurée depuis la version 1 ». | Aucun changement visible dans l’interface. Je n’inspecte aucun fichier externe au navigateur. Vérification du contenu non réalisée. |
| Fichiers → Demandes d’effacement | Je veux contrôler que l’annulation est toujours effective après la restauration. | « Aucune demande d’effacement acceptée. » Archive et Elsewhere sont tous deux à nouveau présents dans Fichiers. |
| Fermeture du navigateur | Le parcours est terminé ; je laisse les services actifs pour une vérification indépendante. | Fermeture confirmée par le navigateur. |

## Ce que je comprends de l’effacement, avec mes mots

Le bouton « Libérer l’espace » n’est pas celui que je choisirais pour que Pigeon cesse de conserver mes notes : j’ai utilisé la demande d’effacement depuis le fichier. L’écran décrit une portée précise : le fichier actuel et les deux versions déjà observées. Le mot « observées » me fait comprendre que l’opération porte sur les versions que Pigeon connaît ici ; je n’ai pas reçu de garantie plus large concernant des versions inconnues.

La préparation et la demande sont deux étapes distinctes. Préparer ne supprime ni ne programme l’effacement. Après la préparation, j’ai dû revenir à l’examen puis demander réellement l’effacement. Cette demande a retiré notes.txt des fichiers actuels : Archive n’était plus dans la liste. Cela n’a pas signifié que toutes les données étaient déjà détruites ; la demande restait en attente et annulable.

Les anciennes versions sont également ciblées, et non seulement le fichier actuel. Le délai donne encore la possibilité de conserver une version en la restaurant avant la fin. En revanche, une fois des octets réellement supprimés, annuler ne peut pas les recréer.

Les 79 octets distincts indiqués ne constituent pas une promesse de libérer 79 octets de stockage. Une des versions sélectionnées utilise des octets également conservés pour Elsewhere/duplicate.txt. Ces octets restent nécessaires à cet autre fichier. J’ai bien vu Elsewhere rester présent après le retrait des notes.

La fin peut arriver au plus tôt après les délais et les accords requis, pas simplement au bout de 30 jours à partir de mon clic. Chaque sauvegarde doit attendre au moins 30 jours après avoir reçu la demande et les sauvegardes requises doivent donner leur accord. La demande affichait « Fin déclarée au plus tôt : 2026-11-15T12:28:40Z », tout en précisant que le délai propre à chaque destinataire s’applique aussi. Family backup était encore attendue. Je ne peux donc pas annoncer une date d’achèvement certaine.

Pour moi, « définitif » signifie que ces versions peuvent devenir irrécupérables dans Pigeon après l’effacement effectif. Cela ne signifie ni nettoyage sécurisé des disques ni suppression de copies déjà faites hors de Pigeon. Les octets partagés encore utiles ailleurs ne disparaissent pas pour cette seule demande.

Annuler la demande et récupérer un fichier sont deux opérations séparées. Après l’annulation, notes.txt était toujours absent des fichiers actuels. J’ai dû le restaurer depuis Fichiers supprimés, puis restaurer la version 1 depuis son historique. Cette restauration a créé la version actuelle 3 et conservé les deux versions précédentes.

## Confusions, erreurs, aide et facilités

- **Préparation en deux temps :** j’avais compris qu’elle n’effaçait pas immédiatement, mais le message « aucun effacement […] programmé » a précisé qu’une seconde demande était indispensable. Le retour faisait apparaître le bon bouton ; aucun indice extérieur n’a été nécessaire.
- **État des machines :** Bob et Family backup étaient initialement attendus. Après Retour, cette attente avait disparu. J’ai supposé la préparation terminée d’après le nouveau bouton, sans indication explicite « toutes les machines sont prêtes » sur l’écran revu.
- **Reçu peu adapté au navigateur :** le renvoi à `pigeon erase` m’a momentanément laissé sans prochaine action évidente. Je suis repassé par la navigation globale. Le reçu portait `bb0a7d5f6d406ccc50a6d96fa83586a81a626adb`, tandis que la demande à annuler portait `7a78f0fa40f8ac66d6d986ae7989ccb058c24f5416ef0c46d6022163d3994de2`. Je n’ai pas compris la relation entre ces identifiants. Le chemin du fichier m’a rassuré.
- **Annulation bien expliquée :** l’avertissement « les fichiers actuels ne sont pas restaurés automatiquement » a rendu l’étape suivante compréhensible. La confirmation d’annulation était claire.
- **Restauration indirecte :** Fichiers supprimés ne donne pas directement accès au choix d’une version. J’ai remis le fichier puis rouvert son historique. Ce détour était trouvable mais plus long que prévu.
- **Message de restauration ambigu au premier passage :** la mention de « la version plus récente » apparaissait même après le bouton générique Restaurer, avant que j’aie sélectionné la version 1. Cela ne m’a pas permis d’identifier le contenu remis.
- **Attente erronée sur l’historique :** je pensais que la version 1 deviendrait simplement la ligne actuelle ; Pigeon a créé une version 3. L’opération avait réussi, mais le lien entre les versions 3 et 1 n’est pas visible dans le texte de cet historique.
- **Vérification sans résultat visible :** « Ouvrir une copie » n’a rien changé dans le navigateur. Je ne sais pas si une application extérieure ou un téléchargement était attendu. Je n’ai pas poursuivi hors de l’interface autorisée.
- **Facilités :** les liens globaux sont restés disponibles ; le fichier et les deux versions étaient faciles à repérer ; le résumé préalable décrit clairement les octets partagés, le délai et les limites du mot définitif ; les confirmations d’annulation et de restauration m’ont permis de continuer sans aide.
- **Erreurs de manipulation :** aucune mauvaise cible ni demande involontaire constatée. Mes erreurs étaient des attentes sur le comportement, décrites ci-dessus.

## Appréciation de facilité

Échelle : 1 = très difficile ; 7 = très facile.

| Partie | Note | Pourquoi |
|---|---:|---|
| Trouver et demander l’effacement du fichier et de l’historique | 5/7 | Cible facile à trouver et explication utile ; préparation puis seconde confirmation à interpréter. |
| Comprendre les conséquences, les octets partagés et la date possible | 6/7 | Résumé préalable précis et limites explicites ; le mot « observées » limite la portée et la date reste conditionnelle. |
| Annuler la demande | 6/7 | Parcours direct depuis la liste et avertissement clair ; identifiant très long et reçu différent. |
| Récupérer la plus ancienne version | 5/7 | Réalisable sans aide, mais restauration préalable du fichier, nouvelle version 3 inattendue et contenu non vérifiable dans le navigateur. |
| Ensemble du parcours | 5/7 | Objectifs atteints d’après l’interface, avec plusieurs moments d’interprétation. |

## Repères de vérification dans les captures produites par le navigateur

- `browser/screenshot-5.png` : portée, 79 octets, partage avec Elsewhere, délais et préparation.
- `browser/screenshot-6.png` : préparation seule, aucun effacement effectué ni programmé.
- `browser/screenshot-7.png` : bouton « Demander cet effacement ».
- `browser/screenshot-8.png` : reçu de la demande.
- `browser/screenshot-10.png` : demande en attente, date au plus tôt et Family backup.
- `browser/screenshot-11.png` : conséquences de l’annulation.
- `browser/screenshot-12.png` : annulation confirmée.
- `browser/screenshot-14.png` : fichier supprimé et boutons de restauration.
- `browser/screenshot-18.png` : version 1 sélectionnable avant restauration explicite.
- `browser/screenshot-19.png` : confirmation de cette restauration.
- `browser/screenshot-20.png` : version 3 actuelle, versions 2 et 1 conservées.
- `browser/screenshot-23.png` : aucune demande d’effacement acceptée.

Ces chemins sont relatifs à `/tmp/pgn-m5u-french`. Les captures ont été produites par l’outil de navigation ; je n’ai pas examiné les fichiers privés de préparation. Le navigateur a été fermé et les démons ont été laissés actifs.
