# Reprise ciblée indépendante — interface française

Session du 17 octobre 2026, après redémarrage du démon, dans `/tmp/pigeon-m6-usability-fr`. Nouvelle connexion à usage unique obtenue et consommée. Évaluation par un agent dans le rôle d’une personne francophone ; ce n’est pas une étude avec un participant humain. Vérification exclusivement par pages et formulaires HTTP réellement servis, sans JavaScript. Aucun code d’implémentation consulté, aucune modification du dépôt, aucun contrôle CLI de configuration et aucune affirmation visuelle ou matérielle.

Les réglages précédents étaient visibles au début : Rapide, envoi `2 MB/s`, plage `22:00-23:00`, batterie `30` avec Attendre, réseau limité avec Continuer.

## Ce qui est désormais compréhensible

**Confirmations des valeurs libres : corrigées dans le parcours HTTP testé.** J’ai réenregistré les trois valeurs conservées. Les réponses HTTP 200 affichent exactement :

- « Envoi maximal (auto ou 2 MB/s) : 2 MB/s. »
- « Plages à pleine vitesse (vide : aucune) : 22:00-23:00. »
- « Seuil de batterie faible (%) : 30. »

Le préfixe `setting-` rencontré dans la session précédente n’apparaît plus.

**Pleine vitesse, plafond et heure locale : explication désormais suffisante pour la tâche.** La page Réglages dit : « Les débits sont en octets par seconde ; séparez les plages HH:MM-HH:MM en heure locale par des virgules. Les plages à pleine vitesse désactivent le ralentissement automatique ; vos limites d’envoi et de réception restent applicables. » Je comprends donc que `22:00-23:00` est une plage en heure locale et que mon envoi restera plafonné à `2 MB/s` pendant cette plage. « Pleine vitesse » n’est plus présenté sans expliquer sa relation avec le plafond.

**Attente batterie/réseau et pause choisie : distinction désormais explicite.** Le nouveau texte dit : « Avec Attendre, les transferts de fichiers attendent sous le seuil de batterie ou en mode économie d’énergie, puis reprennent automatiquement quand les conditions s’améliorent. Les attentes liées au réseau limité prennent aussi fin automatiquement. Reprendre le partage met fin à une pause que vous avez choisie. » Je peux maintenant expliquer que l’attente est conditionnelle et se termine automatiquement, tandis que Reprendre agit sur ma pause volontaire. Avec mon seuil de 30, le texte indique une attente sous ce seuil, ainsi qu’en mode économie d’énergie. Le réglage réseau est actuellement Continuer ; la phrase sur Attendre décrit donc une politique que je pourrais choisir, pas un état réseau actuellement observé.

J’ai demandé une pause d’une heure par le formulaire réel. La réponse disait « Le partage est en pause jusqu’à 09 h 39. » et « Les changements ne sont enregistrés que sur cet ordinateur. » Le bouton « Reprendre le partage » a renvoyé « Le partage reprend. » Les pages suivantes conservaient la politique batterie et son seuil. Cette action vérifie le parcours pause/reprise choisi ; elle ne démontre pas une attente matérielle batterie ou réseau.

**Débit invalide et retour : aide désormais adaptée à la page.** J’ai soumis `banane` dans Envoi maximal. La réponse HTTP 200 disait exactement : « « banane » ne convient pas pour Envoi maximal (auto ou 2 MB/s). Rien n’a changé. Revenez en arrière et utilisez un choix proposé ou le format indiqué pour ce réglage. » Elle affichait un lien « Retour » vers `/settings`. Aucun `resources.upload`, code d’erreur technique ou renvoi au CLI n’apparaissait dans cette réponse.

Après Retour, le champ affichait bien `2 MB/s`, la plage `22:00-23:00`, le seuil `30` et Attendre pour la batterie. J’ai ensuite réenregistré `2 MB/s` avec succès. La promesse « Rien n’a changé » est donc cohérente avec les valeurs réaffichées.

## Ambiguïtés et limites restantes

- La confirmation isolée « Le partage reprend. » reste plus générale que l’explication de Réglages : elle ne rappelle pas que les conditions d’alimentation ou de réseau peuvent encore faire attendre les transferts. Le texte de Réglages permet cependant maintenant de comprendre la distinction ; ce point n’a pas empêché la tâche.
- La page d’erreur remplace toujours le formulaire : il faut Retour pour corriger. Le lien est explicite et l’exemple `2 MB/s` est repris dans l’erreur ; la récupération testée est claire.
- « Heure locale » est explicite mais le fuseau précis n’est pas affiché. Je n’ai pas testé un changement de fuseau ni un navigateur dans un autre fuseau ; je n’en déduis aucun défaut d’exécution.
- Aucun événement batterie faible, économie d’énergie ou réseau limité n’a été déclenché. Les conclusions sur reprise automatique et priorité du plafond portent sur la compréhension du texte, pas sur une validation matérielle ou une mesure de débit.

Les blocages de compréhension sur pleine vitesse/plafond, les préfixes de confirmation et le renvoi technique de l’erreur initiale sont levés dans ce parcours HTTP français.
