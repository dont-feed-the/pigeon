# Reprise courte — refus français de système de fichiers

17 octobre 2026. Nouveau domicile isolé, langue française configurée, locale CLI explicitement française, relais désactivés. Binaire reconstruit fourni, utilisé sans accès au code d’implémentation ni modification du dépôt. Le premier rapport est conservé.

Commande réellement tentée : `pigeon create "Essai du refus" /proc/pigeon-usability-fr-89d0794d67/folder`.

La commande a renvoyé le code de sortie 1. Texte exact :

> Le système de fichiers de ce dossier ne permet pas à pigeon de travailler en sécurité. Cette vérification ne modifie pas vos fichiers. Choisissez un dossier local sur ext4, btrfs ou xfs sous Linux, ou APFS sous macOS. Les machines de sauvegarde Linux peuvent aussi utiliser ZFS.
> Code : P-FILE-SYSTEM-UNSUPPORTED

Copie exacte de la sortie : `/tmp/pigeon-filesystems-usability-fr-retake-output.txt` ; détails de l’exécution : `/tmp/pigeon-filesystems-usability-fr-retake-runtime.json`.

**Compréhension :** le refus reste clair, la prochaine action reste de choisir un dossier local sur un système indiqué, et « Cette vérification ne modifie pas vos fichiers » rassure désormais explicitement sur l’absence de modification pendant le contrôle. La lacune de réassurance signalée au premier essai est levée dans ce message. La phrase est courte, compréhensible et bien placée avant l’instruction suivante.

**Limite restante :** les noms ext4/btrfs/xfs peuvent demander une vérification à une personne débutante ; le message seul n’explique pas cette vérification et ne fournit pas de lien vers le guide. Le guide et ses instructions df/Disk Utility n’ont pas été évalués dans cette reprise exclusivement à l’exécution.

Aucun fichier utilisateur préexistant n’était présent dans la cible unique : ce constat porte sur la clarté de la promesse, pas sur une preuve matérielle de conservation. Aucun essai de création acceptée, d’annex ou de compatibilité matérielle.

**Nettoyage :** démon lancé sous `reap-run`, signal TERM envoyé au démon, puis attente de la fin du wrapper (sortie 0). Tous les PID enregistrés ont disparu de `/proc` ; aucun enfant ni zombie de cette reprise ne reste.
