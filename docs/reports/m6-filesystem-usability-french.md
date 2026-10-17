# Essai ciblé des refus de système de fichiers — français

Session du 17 octobre 2026, par un agent dans le rôle d’un utilisateur francophone. Binaire actuel `/workspace/pigeon-target-files/debug/pigeon`, démon isolé `/tmp/pigeon-fs-usability-fr-dafec52da1`, `language = "fr"`, relais désactivés. Aucun code d’implémentation consulté ni modifié ; seules les nouvelles chaînes Fluent françaises ont été lues après l’essai.

Commande réellement tentée :

`pigeon create "Essai du refus" /proc/pigeon-usability-fr-1d8fb94627/folder`

Avec la locale CLI française, la commande s’est terminée avec le code 1 et a affiché exactement :

> Le système de fichiers de ce dossier ne permet pas à pigeon de travailler en sécurité. Choisissez un dossier local sur ext4, btrfs ou xfs sous Linux, ou APFS sous macOS. Les machines de sauvegarde Linux peuvent aussi utiliser ZFS.
> Code : P-FILE-SYSTEM-UNSUPPORTED

- **Refus : clair.** Le problème est attribué au système de fichiers du dossier et à l’impossibilité de travailler en sécurité. Je comprends que je dois choisir un autre emplacement avant de poursuivre.
- **Prochaine action : concrète, mais technique.** « Choisissez un dossier local » et les systèmes acceptés donnent une direction utilisable. Une personne qui ne connaît pas ext4/btrfs/xfs peut encore ignorer où trouver ce dossier ou comment vérifier son système de fichiers. Le texte ne demande pas de reformater, supprimer ou déplacer ses fichiers ; l’action proposée est un nouveau choix d’emplacement.
- **Conservation des fichiers existants : non explicitée.** Aucun équivalent de « Vos fichiers existants sont restés en place » ou « Rien n’a changé » n’apparaît. Un utilisateur peut donc comprendre le refus sans être certain de ce qui s’est passé avant le refus. C’est la principale lacune rédactionnelle de cet essai. L’emplacement `/proc/...` unique ne contenait aucun fichier utilisateur ; cet essai ne valide pas matériellement la conservation de fichiers préexistants. Le dossier cible n’existait toujours pas après le refus.

La chaîne Fluent `problem-file-system-unknown`, lue mais non déclenchée, indique : « pigeon n’a pas pu vérifier le système de fichiers de ce dossier. Vérifiez que le dossier ou son parent est accessible, puis réessayez. » Elle propose une action de récupération compréhensible, mais ne rassure pas non plus explicitement sur la conservation des fichiers.

Note de locale : la première invocation avait hérité de `LC_ALL=C.UTF-8` et affichait l’anglais malgré la configuration française du démon. Après réglage de la locale CLI en français, l’erreur ci-dessus était française. Ce n’est pas compté comme un défaut du texte de refus.

Nettoyage : le démon isolé a été arrêté et aucun enfant n’avait été lancé. Aucun processus actif de ce groupe ne reste ; son PID apparaît uniquement comme zombie déjà terminé, en attente de collecte par PID 1. Aucun essai de création acceptée ou d’annex, ni affirmation de comportement matériel, de compatibilité mesurée ou de support supplémentaire.
