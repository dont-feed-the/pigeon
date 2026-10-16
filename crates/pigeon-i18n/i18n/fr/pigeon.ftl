# pigeon's French messages (ADR 0028). The words follow the French column of
# docs/design/ux.md §2.2. The ids are those of ../en/pigeon.ftl, listed in
# crates/pigeon-i18n/src/ids.rs; a test checks that every bundle agrees.

## Les états d'une version (ux.md §2.1, ADR 0027)

state-local = Seulement sur cet ordinateur
state-published = Partagé, pas encore en sécurité
state-protected = En sécurité
state-uncertain = Pas vérifié depuis { $since }
state-missing = Aucune copie n'est trouvée
state-erased = Effacé

## Niveaux du titre (ux.md §4)

level-needs-you = À vous d'agir
level-not-safe = Pas encore en sécurité
level-working = En cours
level-safe = En sécurité
level-not-checked = Pas vérifié récemment

## Titres : quoi, puis pourquoi et la suite (ux.md §4)

headline-safe = Tout est en sécurité. Vérifié { $ago }.
headline-capturing =
    { $files ->
        [one] Vérification d'un fichier modifié ; il n'est pas encore en sécurité.
       *[other] Vérification de { $files } fichiers modifiés ; ils ne sont pas encore en sécurité.
    }
headline-checking = Le dossier local n'a pas pu être vérifié. Lancez : pigeon doctor
headline-too-few-backups = Pas encore en sécurité : ajoutez un ordinateur de sauvegarde.
why-capturing = Cette modification attend d'être enregistrée et copiée sur les ordinateurs de sauvegarde.
why-too-few-backups = { $have } ordinateurs de sauvegarde sur { $need } sont configurés. Pour en ajouter un : pigeon invite --backup-machine
headline-not-safe =
    { $files ->
        [one] { $files } fichier n'est pas encore en sécurité.
       *[other] { $files } fichiers ne sont pas encore en sécurité.
    }
headline-working =
    { $files ->
        [one] Envoi de { $files } fichier aux machines de sauvegarde…
       *[other] Envoi de { $files } fichiers aux machines de sauvegarde…
    }
headline-not-checked = Dernière confirmation de sécurité : { $when }. pigeon ne peut pas joindre le groupe en ce moment ; les changements depuis ne sont que sur cet ordinateur.
headline-disk-full = Votre disque est plein. pigeon ne peut pas enregistrer les nouveaux changements. Libérez de l'espace ou videz la corbeille.
headline-no-capacity = Cette machine de sauvegarde n'a pas la place pour { $files } fichiers. Il lui faut { $missing_mb } Mo d'espace libre en plus.
headline-machine-missing =
    { $days ->
        [one] { $machine } est introuvable depuis { $days } jour.
       *[other] { $machine } est introuvable depuis { $days } jours.
    } Remplacez-la ou ajoutez une autre machine de sauvegarde.
headline-machine-full = { $machine } est pleine ; elle attend qu'on y libère de l'espace.
headline-machine-no-capacity = { $machine } n'a pas la place pour { $files } fichiers : il lui faut { $missing_mb } Mo d'espace libre en plus. Libérez-y de l'espace, ou ajoutez une autre machine de sauvegarde.
headline-machine-lost = { $machine } ne trouve plus son dossier de sauvegarde ; elle ne compte plus jusqu'à ce qu'elle soit réinstallée.
why-machine-offline = { $machine } est hors ligne depuis { $since }.
why-safe-copies = { $have } sur { $need } copies de sauvegarde
next-when-back = Rien à faire : ils seront en sécurité quand { $machine } sera de retour.
next-repairing = Réparation en cours : { $files } fichiers sont copiés sur une autre machine de sauvegarde.

## Temps relatifs (ux.md §4) ; les temps absolus viennent de jiff

time-just-now = à l'instant
time-minutes-ago =
    { $minutes ->
        [one] il y a { $minutes } minute
       *[other] il y a { $minutes } minutes
    }
time-hours-ago =
    { $hours ->
        [one] il y a { $hours } heure
       *[other] il y a { $hours } heures
    }
time-days-ago =
    { $days ->
        [one] il y a { $days } jour
       *[other] il y a { $days } jours
    }
time-never = jamais

## Commandes de la tranche de sécurité (13 §13.12)

cmd-create = Créer un groupe dont cet ordinateur est le premier membre
cmd-invite-person = Afficher une invitation à usage unique pour l'ordinateur d'une personne, valable 24 heures
cmd-invite = Afficher une invitation à usage unique, valable 24 heures
cmd-join = Rejoindre un groupe avec une invitation
cmd-status = Dire si vos fichiers sont en sécurité, et pourquoi sinon
cmd-machines = Lister les ordinateurs et les machines de sauvegarde du groupe
cmd-open = Ouvrir pigeon dans votre navigateur
cmd-history = Lister les versions d'un fichier
cmd-restore = Restaurer une version, ou un fichier supprimé
cmd-free = Libérer l'espace d'un dossier sur cet ordinateur
cmd-keep = Garder un dossier sur cet ordinateur
cmd-suggestions = Lister les suggestions qui attendent une décision
cmd-accept = Accepter une suggestion dans les fichiers du groupe
cmd-decline = Refuser une suggestion, en disant pourquoi si vous voulez
cmd-conflicts = Lister vos fichiers qui diffèrent de la version du groupe
cmd-resolve = Choisir pour un fichier qui diffère : la version du groupe, la vôtre en copie, ou la suggérer à nouveau
cmd-remove-computer = Retirer du groupe un ordinateur perdu ou mis à la retraite
cmd-recovery-sheet = Afficher la fiche de récupération de ce groupe
cmd-recover = Récupérer le groupe sur un nouvel ordinateur depuis ses machines de sauvegarde
cmd-doctor = Vérifier l'installation et expliquer ce qui ne va pas
cmd-debug-bundle = Rassembler un diagnostic pour la personne qui vous aide
cmd-update = Installer la dernière version de pigeon, une version donnée, main compilé depuis les sources, ou la précédente
opt-json = Afficher le résultat en JSON, pour les scripts
opt-backup-machine = Pour une machine de sauvegarde plutôt que l'ordinateur d'une personne
opt-not-shared = Lister ce qui n'est pas partagé, et pourquoi
help-usage = Usage : pigeon COMMANDE [ARGUMENTS]
help-commands = Commandes :

## Résultats

free-result = { $safe } fichiers sur { $total } sont en sécurité et leur espace est libéré.
free-waiting =
    { $files ->
        [one] { $files } fichier n'est pas encore en sécurité ; pigeon le libérera dès qu'il le sera.
       *[other] { $files } fichiers ne sont pas encore en sécurité ; pigeon les libérera dès qu'ils le seront.
    }
keep-result = { $folder } est gardé sur cet ordinateur.
suggestions-none = Aucune suggestion n'attend.
suggestion-decide = { $id }  { $author } suggère une modification de { $paths }, enregistrée { $saved }. Acceptez-la avec : pigeon accept { $id }
suggestion-other = { $id }  { $author } suggère une modification de { $paths }, enregistrée { $saved } ; quelqu'un d'autre décide.
suggestion-yours = { $id }  Votre modification de { $paths }, enregistrée { $saved }, attend une décision.
accepted-suggestion = La suggestion de { $author } est maintenant dans les fichiers du groupe.
declined-suggestion = La suggestion de { $author } est refusée ; la personne sera prévenue.
conflicts-none = Chaque fichier ici est la version du groupe.
conflict-waiting = { $path } : votre suggestion attend une décision.
conflict-choose = { $path } : votre version diffère de celle du groupe. Choisissez avec : pigeon resolve
resolved-take-group = { $path } est de nouveau la version du groupe ; la vôtre reste dans son historique.
resolved-keep-copy = Votre version de { $path } est gardée sous { $dest } ; { $path } est de nouveau la version du groupe.
resolved-suggest-again = Votre version de { $path } est suggérée à nouveau.
undo-free = Annuler : pigeon keep { $folder }
undo-keep = Annuler : pigeon free { $folder }
restore-result = { $path } est restauré ; la version plus récente reste dans son historique.
recovery-sheet-text = { $sheet }
recovered = Cet ordinateur est de retour dans { $group }. Ses fichiers arrivent depuis les machines de sauvegarde ; retirez ensuite les ordinateurs perdus, révoquez la fiche utilisée avec remove-computer et imprimez une nouvelle fiche.
not-shared-kind = { $path } : non partagé ({ $kind })
doctor-lab-build = Ceci est une version de laboratoire : ses rapports d'état sont marqués, et aucune version publiée ne les compte.
doctor-version = { $program } { $found } (il faut au moins { $minimum })
doctor-ok = Tout ce dont pigeon a besoin est en place.
created = Le groupe « { $group } » est créé. Ajoutez une machine de sauvegarde avec : pigeon invite --backup-machine
invitation =
    { $who ->
        [backup-machine] Sur la machine de sauvegarde, dans les 24 heures, lancez : pigeon join --backup-machine { $code }
       *[computer] Sur le nouvel ordinateur, dans les 24 heures, lancez : pigeon join { $code } DOSSIER
    }
joined =
    { $who ->
        [backup-machine] Cette machine de sauvegarde garde maintenant des copies pour « { $group } ».
       *[computer] Cet ordinateur fait maintenant partie de « { $group } ». Les fichiers du groupe arrivent dans son dossier.
    }
removed-computer = { $name } est retiré du groupe ; ses copies ne comptent plus.
bundle-written = Le diagnostic est dans { $path }. Rien n'a été envoyé.
machine-row = { $name } : { $kind }, dernier contact { $heard }
machine-kind-custodian = machine de sauvegarde
machine-kind-computer = ordinateur
person-row = { $name } ({ $role }) : { $machines }
person-admin = admin
person-member = membre
invitations-none = Aucune invitation n'attend.
invitation-row = { $id } : pour { $who }, jusqu'au { $expires }
invitation-row-backup = { $id } : une machine de sauvegarde de { $who }, jusqu'au { $expires }
invitation-cancelled = L'invitation { $id } ne peut plus servir.
cmd-people = Les membres du groupe et leurs ordinateurs
cmd-deleted = Les fichiers supprimés du groupe, que restore fait revenir
machine-this = { $name } : cet ordinateur
version-row = { $number }. { $saved } : { $state }
waiting-row = { $path } : { $state }
confirm-remove-computer = Retirer { $name } du groupe ? Ses copies ne compteront plus. [o/N]
confirm-yes = oui
confirm-leave = Retirer cet ordinateur du groupe ? Ses fichiers restent dans le dossier, mais il cesse de les partager. [o/N]
confirm-refound = Refonder le groupe avec vous pour seul administrateur ? Il devient un nouveau groupe : chaque autre membre le rejoint à nouveau par une invitation de votre part, et l'ancien historique reste lisible. [o/N]
confirm-join-refounded = { $group } a été refondé par { $name } le { $date } : le rejoindre, c'est accepter { $name } comme administrateur. Rejoindre ? [o/N]
confirm-held-share = Supprimer les fichiers de { $batch } pour tout le monde ? Ils restent dans l'historique. [o/N]

## Problèmes (10 §10.6, ADR 0023) : problem-<code>, et hint-<code> s'il y en a un

problem-code = Code : { $code }
problem-not-in-this-build = « { $command } » n'est pas encore dans cette version de pigeon.
hint-not-in-this-build = Cette première version n'a que les commandes qui mettent les fichiers en sécurité ; les autres viendront.
problem-file-system-not-in-this-build = Ce dossier est sur { $fs }, que cette version de pigeon ne prend pas encore en charge.
hint-file-system-not-in-this-build = Choisissez un dossier sur ext4 (Linux) ou APFS (macOS).
problem-invite-expired = Cette invitation a expiré (elle était valable jusqu'à { $until }).
hint-invite-expired = Demandez-en une nouvelle à { $inviter }.
problem-invite-used = Cette invitation a déjà servi.
hint-invite-used = Demandez-en une nouvelle à { $inviter }.
problem-invite-unknown = Le groupe ne connaît pas cette invitation.
problem-invite-not-allowed = { $inviter } ne peut plus inviter dans ce groupe.
problem-inviter-offline = L'ordinateur de { $inviter } est injoignable pour le moment.
hint-inviter-offline = Réessayez quand l'ordinateur de { $inviter } est allumé.
problem-not-in-group = Cet ordinateur n'est pas membre du groupe.
problem-not-allowed = Vous n'avez pas le droit de faire cela dans ce groupe.
problem-suggestion-unknown = Aucune suggestion en attente ne s'appelle { $id }.
hint-suggestion-unknown = Voyez-les avec : pigeon suggestions
problem-not-diverging = { $path } est déjà la version du groupe.
hint-not-diverging = Voyez les fichiers qui diffèrent avec : pigeon conflicts
problem-changed-since = { $path } a été enregistré de nouveau depuis ; rien n'a été remplacé.
hint-changed-since = Votre nouvel enregistrement est suggéré à son tour ; choisissez de nouveau quand vous serez prévenu·e.
problem-dest-exists = { $path } existe déjà ; choisissez un autre nom pour la copie.
problem-needs-override = Personne ne peut faire cette modification ; un·e administrateur·rice peut quand même l'accepter, en disant pourquoi.
hint-needs-override = Lancez : pigeon accept ID --override "la raison"
problem-needs-confirmation = Ceci demande votre confirmation, et pigeon ne peut pas la demander ici.
hint-needs-confirmation = Relancez-le dans un terminal, ou ajoutez --yes.
problem-backup-machine-needs-linux = Une machine de sauvegarde doit tourner sous Linux avec systemd.
problem-disk-full = { $machine } est pleine.
hint-disk-full = Libérez-y de l'espace, ou ajoutez une autre machine de sauvegarde.
problem-no-capacity = { $machine } n'a pas la place pour { $files } fichiers : il lui faut { $missing_mb } Mo d'espace libre en plus.
hint-no-capacity = Libérez-y de l'espace, ou ajoutez une autre machine de sauvegarde.
problem-commit-unsigned = Une modification venant de { $machine } n'est pas signée ; elle a été refusée ({ $commit }).
problem-commit-forged = Une modification venant de { $machine } a une signature qui ne lui correspond pas ; elle a été refusée ({ $commit }).
problem-commit-unknown-signer = Une modification venant de { $machine } a été signée par un ordinateur hors du groupe ; elle a été refusée ({ $commit }).
problem-commit-not-allowed = Une modification venant de { $machine } change ce que sa personne n'a pas le droit de changer ; elle a été refusée ({ $commit }).
problem-commit-malformed = Une modification venant de { $machine } est mal formée ; elle a été refusée ({ $commit }).
problem-commit-waits = Une modification venant de { $machine } attend une version plus récente de pigeon ({ $commit }).
problem-repository-lost = Les fichiers de { $machine } ont été perdus ou remplacés.
problem-daemon-not-running = pigeon ne tourne pas sur cet ordinateur.
hint-daemon-not-running = Lancez pigeon open pour le démarrer, puis vérifiez avec : pigeon doctor
problem-version-mismatch = Cette commande pigeon et le pigeon qui tourne ne sont pas de la même version.
hint-version-mismatch = Redémarrez pigeon.
problem-not-installed = Ce pigeon n'a pas été installé depuis une version publiée ; il ne peut pas se mettre à jour lui-même.
hint-not-installed = Installez une version publiée avec : curl -fsSL https://github.com/mariogeiger/pigeon/releases/latest/download/pigeon-install | sh
problem-git-annex-too-old = git-annex { $found } est trop ancien ; pigeon a besoin de { $minimum } ou plus récent.
problem-git-annex-untested = git-annex { $found } est plus récent que les versions avec lesquelles pigeon a été testé.
problem-update-needed = Le groupe utilise un pigeon plus récent. Mettez pigeon à jour sur cet ordinateur.
problem-pigeon-too-old = Ce pigeon est trop ancien pour le groupe.
problem-peer-too-old = { $peer } utilise un pigeon trop ancien pour le groupe.
problem-peer-too-new = { $peer } utilise un pigeon plus récent que celui-ci ne peut pas suivre.
problem-settings-check = Les réglages du groupe sur cet ordinateur ont été modifiés hors de pigeon ; la libération d'espace attend qu'ils soient rétablis.
problem-receiving = Une copie de ce dossier est encore en train d'arriver.
problem-no-such-version = { $path } n'a pas de version { $version }.
problem-sheet-unreadable = Ce n'est pas une fiche de récupération, ou elle a été mal recopiée.
hint-receiving = Réessayez dans un instant.
problem-unknown-computer = Le groupe n'a aucun ordinateur nommé { $name }.
hint-unknown-computer = Voyez les noms avec : pigeon machines
problem-usage = « { $command } » a reçu de mauvais arguments.
hint-usage = Voir : pigeon help
problem-not-a-pigeon-repository = Ce dossier n'est pas partagé avec pigeon.
problem-peer-unreachable = L'autre machine est injoignable pour le moment.
problem-peer-refused = L'autre machine a refusé la connexion.
problem-internal-error = Quelque chose s'est mal passé dans pigeon.
hint-internal-error = Rassemblez les détails avec : pigeon debug bundle

## L'interface web (10 §10.3)
web-title = pigeon
web-nav-home = Accueil
web-nav-suggestions = Suggestions
web-nav-machines = Membres et machines
web-forbidden = Ouvrez pigeon depuis cet ordinateur avec : pigeon open
web-login-used = Ce lien a déjà servi. Ouvrez pigeon de nouveau avec : pigeon open
web-login = Ouvrir pigeon dans votre navigateur : { $url }
web-suggestion-decide = { $author } suggère une modification de { $paths }, enregistrée { $saved }.
web-suggestion-other = { $author } suggère une modification de { $paths }, enregistrée { $saved } ; quelqu'un d'autre décide.
web-suggestion-yours = Votre modification de { $paths }, enregistrée { $saved }, attend une décision.
web-accept = Accepter
web-decline = Refuser
web-decline-note = Pourquoi (facultatif)
web-back = Retour
web-form-expired = Cette page n'est plus à jour. Rechargez-la et réessayez.
web-nav-files = Fichiers
web-root-files = Fichiers dans le dossier partagé
web-setup = Rejoindre ou créer un groupe
web-setup-help = Demandez une invitation à un membre du groupe, ou créez un groupe pour votre famille ou votre équipe. Choisissez un chemin de dossier absolu sur cet ordinateur. Un ordinateur de sauvegarde n'a pas besoin de dossier partagé.
web-join = Rejoindre le groupe
web-create = Créer un groupe
web-invitation = Invitation
web-invitation-ready = Invitation prête
web-invite-handoff = Envoyez cette invitation à la personne qui rejoint le groupe. Sur l'autre ordinateur, installez et ouvrez pigeon, choisissez Rejoindre ou créer un groupe, puis collez le code dans les 24 heures. Gardez cet ordinateur connecté jusqu'à la fin de l'adhésion.
web-invitation-code = Code d'invitation
web-command-line = Instructions en ligne de commande
web-local-folder = Dossier sur cet ordinateur
web-computer-name = Nom de cet ordinateur
web-group-name = Nom du groupe
web-backup-copies = Nombre d'ordinateurs de sauvegarde nécessaires à la sécurité
status-freshness = Une vérification de sécurité est récente pendant une heure. Si aucun ordinateur de sauvegarde n'a donné de nouvelles depuis une heure, pigeon indique que la vérification n'est plus récente. Cela ne signifie pas que les fichiers sont perdus. Un ordinateur silencieux pendant 24 heures ne compte plus parmi les copies de sauvegarde. Une nouvelle modification peut afficher Envoi pendant dix minutes ; elle n'est en sécurité que lorsque suffisamment d'ordinateurs de sauvegarde la conservent.
web-keep = Garder sur cet ordinateur
web-free = Libérer l'espace
folders-none = Le groupe n'a encore aucun dossier.
folder-row =
    { $files ->
        [one] { $path } : { $state }, { $files } fichier
       *[other] { $path } : { $state }, { $files } fichiers
    }
folder-row-freed =
    { $files ->
        [one] { $path } : { $state }, { $files } fichier ; son espace est libéré sur cet ordinateur
       *[other] { $path } : { $state }, { $files } fichiers ; leur espace est libéré sur cet ordinateur
    }
web-nav-settings = Réglages
web-save = Enregistrer
web-setting-saved = { $key } : { $value }.
setting-row = { $key } = { $value }
setting-changed = { $key } vaut maintenant { $value }.
setting-language = Langue
setting-notifications = Notifications
setting-auto = Comme cet ordinateur
setting-true = Activées
setting-false = Désactivées
language-en = English
language-fr = Français
cmd-settings = Afficher ou changer les réglages de cet ordinateur
problem-setting-unknown = « { $key } » n'est pas un réglage que pigeon connaît.
hint-setting-unknown = Voyez-les avec : pigeon settings
problem-setting-value = « { $value } » n'est pas une valeur possible de « { $key } ».
hint-setting-value = Voyez les valeurs avec : pigeon settings
cmd-pause = Mettre le partage en pause un moment (par exemple 2h), ou jusqu'à resume
cmd-resume = Reprendre le partage après une pause
headline-paused = Le partage est en pause jusqu'à { $until }. Les changements ne sont enregistrés que sur cet ordinateur.
headline-paused-until-resume = Le partage est en pause. Les changements ne sont enregistrés que sur cet ordinateur jusqu'à la reprise.
paused-until = Le partage est en pause jusqu'à { $until }.
paused-until-resume = Le partage est en pause jusqu'à ce que vous le repreniez.
resumed = Le partage reprend.
cmd-leave = Retirer cet ordinateur du groupe ; ses fichiers restent
leaving = { $name } quitte le groupe dès qu'une autre machine a son retrait. Ses fichiers restent dans le dossier.
leaving-already = Cet ordinateur quitte déjà le groupe.
cmd-refound = Refonder le groupe avec vous pour seul administrateur, quand aucun administrateur ne peut signer
refounded = { $group } est refondé avec { $name } pour seul administrateur. Invitez à nouveau les autres : leurs dossiers le rejoignent sur place.
cmd-held = Changements que pigeon retient jusqu'à votre choix : suppressions à partager ou annuler, noms à renommer
held-none = Rien n'est retenu.
held-row = { $id } : { $reason } ({ $files } fichiers, depuis { $since }) : { $paths }
held-mass-deletion = { $files } fichiers supprimés d'un coup
held-top-folder = le dossier { $folder } supprimé
held-shared = { $files } suppressions vont atteindre les autres ordinateurs.
held-brought-back = { $files } fichiers reviennent.
held-name = le nom { $path } ne marche pas sur tous les ordinateurs ; nom suggéré : { $suggested }
held-name-long = le chemin { $path } est trop long pour certains ordinateurs ; renommez-le
held-renamed = { $from } s'appelle maintenant { $to }.
headline-held-names = { $files } fichiers attendent un nom valable sur tous les ordinateurs.
headline-held = { $files } fichiers supprimés attendent votre choix : les supprimer pour tout le monde, ou les faire revenir ?
web-pause = Mettre en pause
web-resume = Reprendre le partage
pause-hour = pendant une heure
pause-day = pendant un jour
pause-forever = jusqu'à ce que je reprenne
problem-pause-duration = « { $duration } » n'est pas une durée que pigeon comprend.
hint-pause-duration = Écrivez des minutes, des heures ou des jours, par exemple 45m, 2h ou 3d.
problem-restore-at = { $path } n'a pas de version enregistrée au plus tard à « { $at } ».
hint-restore-at = Donnez une date comme 2026-03-03, ou une heure comme 2026-03-03T18:00:00+01:00, après sa première version ; pigeon history les liste.
problem-leave-last-admin = Cet ordinateur est le dernier ordinateur admin du groupe : il ne peut pas partir.
hint-leave-last-admin = Nommez d'abord une autre personne admin, ou ajoutez un autre de vos ordinateurs.
problem-held-unknown = Aucun lot retenu n'a l'ID { $id }.
hint-held-unknown = Voyez-les avec : pigeon held
problem-held-choice = Le lot { $id } n'accepte pas ce choix.
hint-held-choice = Voyez ses choix avec : pigeon held
problem-held-rename-taken = { $to } existe déjà.
hint-held-rename-taken = Renommez-le vous-même ; pigeon le capture dès que son nom marche partout.
web-remove = Retirer
web-remove-confirm = Retirer { $name } du groupe ? Ses copies ne compteront plus.
web-invite = Inviter
web-invite-person = Le nom de la personne
web-invite-admin = comme admin
web-invite-backup = Inviter une machine de sauvegarde
web-invitations = Invitations en attente
web-cancel-invitation = Annuler
files-none = Ce dossier n'a encore aucun fichier.
file-row = { $path } : { $state }
web-restore = Restaurer cette version
web-view = Ouvrir une copie
web-versions = Versions
deleted-none = Aucun fichier n'a été supprimé.
deleted-row = { $path }, supprimé { $deleted }
web-deleted = Fichiers supprimés
web-restore-deleted = Restaurer
notify-needs-you = pigeon a besoin de vous : ouvrez-le pour voir quoi faire.
notify-not-safe-long = Des fichiers ne sont pas en sécurité depuis plus d'un jour.
notify-waiting = Une suggestion ou un conflit vous attend.
notify-open = Ouvrir pigeon
cmd-export = Écrire les fichiers en fichiers ordinaires, sans pigeond, réseau ni clé
export-done =
    { $files ->
        [one] 1 fichier écrit dans { $dir }, à la version { $commit }.
       *[other] { $files } fichiers écrits dans { $dir }, à la version { $commit }.
    } Leur liste est MANIFEST.tsv.
export-deleted =
    { $files ->
        [one] 1 fichier supprimé est dans { $folder }.
       *[other] { $files } fichiers supprimés sont dans { $folder }.
    }
export-missing =
    { $files ->
        [one] 1 fichier n'est pas dans ces dépôts
       *[other] { $files } fichiers ne sont pas dans ces dépôts
    } ({ $damaged } abîmés) ; ils sont peut-être sur d'autres machines de sauvegarde. Leur liste est { $list }.
problem-export-not-a-repository = { $path } n'est ni un dossier pigeon ni le dépôt d'une machine de sauvegarde.
hint-export-not-a-repository = Donnez le dossier, ou le dépôt d'une machine de sauvegarde.
problem-export-no-version = Aucune version « { $at } » dans ces dépôts.
hint-export-no-version = Donnez un commit, ou une date comme 2026-10-01.
problem-export-not-empty = { $dir } n'est pas vide.
hint-export-not-empty = Choisissez un nouveau dossier.
web-held = Changements en attente de votre choix
web-held-share = Supprimer pour tout le monde
web-held-bring-back = Les faire revenir
web-held-rename = Renommer en { $suggested }
web-held-share-confirm = Supprimer les fichiers de { $batch } pour tout le monde ? Ils restent dans l'historique.
web-leave = Quitter le groupe
web-leave-confirm = Retirer cet ordinateur du groupe ? Ses fichiers restent dans le dossier, mais il cesse de les partager.
cmd-free-without-checking = Retirer la copie de cet ordinateur même si c'est la dernière
drop-risk =
    { $files ->
        [one] 1 fichier
       *[other] { $files } fichiers
    } : { $unsafe } pas sur assez de machines de sauvegarde, { $alone } sur aucun autre ordinateur.
drop-preview = Le contenu de ces { $files } fichiers quitterait cet ordinateur, même là où c'est la seule copie.
drop-done = { $files } fichiers masqués, { $dropped } contenus retirés de cet ordinateur ; une trace en est partagée avec le groupe.
confirm-drop = Retirer quand même la copie de cet ordinateur ? Ce qu'aucune autre machine n'a est perdu. [o/N]
cmd-view = Ouvrir une copie en lecture seule d'une version d'un fichier, hors du dossier
view-ready = Une copie en lecture seule est dans { $path } ; elle est supprimée après sept jours.

setting-telemetry-device = Partager les diagnostics avec
setting-off = Garder les diagnostics sur chaque ordinateur
problem-join-history-unverified = Pigeon n’a pas pu vérifier l’historique de ce groupe. Demandez à la personne qui vous a invité de mettre pigeon à jour et de vous envoyer une nouvelle invitation.

problem-recovery-not-allowed = Cette fiche de récupération ne peut plus ajouter d’ordinateurs. Utilisez une fiche actuelle ou demandez une invitation à un administrateur.

machine-recovery = { $name } : clé de récupération sur papier
web-revoke-sheet = Révoquer la fiche de récupération

revoked-sheet = { $name } est révoquée ; cette fiche de récupération ne peut plus ajouter d’ordinateurs.
web-revoke-confirm = Révoquer { $name } ? Cette fiche de récupération ne pourra plus ajouter d’ordinateurs.

progress-restore = Restauration de { $path } ; attente des vérifications du groupe et des copies disponibles.
progress-remove = Retrait de { $name } ; attente des vérifications du groupe.
progress-recovery-sheet = Préparation d’une fiche de récupération et attente de sa réception par une machine de sauvegarde.
progress-recover = Récupération de cet ordinateur ; vérification de l’historique de la sauvegarde et de l’autorisation du nouvel ordinateur.

confirm-revoke-sheet = Révoquer { $name } ? Cette clé sur papier ne pourra plus ajouter d’ordinateurs. [o/N]

cmd-debug-allow = Autoriser un diagnostic pour un ordinateur choisi pendant dix minutes
debug-allowed = { $name } peut recueillir les diagnostics de ce groupe une seule fois, noms de fichiers inclus. Cette autorisation expire dans dix minutes. Redémarrer pigeon annule cette autorisation.
problem-debug-not-allowed = { $name } n’a pas autorisé de diagnostic. Demandez à sa personne d’ouvrir les réglages et d’autoriser un diagnostic pour cet ordinateur, puis réessayez.
confirm-debug-bundle = Enregistrer les diagnostics, noms de fichiers inclus, dans un fichier local privé ? Les autres ordinateurs ne partagent qu’avec leur propre autorisation. [o/N]
confirm-debug-allow = Autoriser { $name } à recueillir une partie des diagnostics de ce groupe, noms de fichiers inclus, pendant les dix prochaines minutes ? [o/N]
progress-debug-bundle = Collecte des diagnostics ; les ordinateurs choisis ne partagent qu’avec l’autorisation de leur personne.
web-diagnostics = Partager un diagnostic une fois
web-diagnostics-consent = J’accepte de partager une fois les diagnostics de ce groupe, noms de fichiers inclus, pendant les dix prochaines minutes avec l’ordinateur choisi.
web-diagnostic-peer = Ordinateur recevant le diagnostic
web-allow-diagnostics = Autoriser un diagnostic

action-cancelled = Annulé.

recovery-title = Fiche de récupération pigeon
recovery-group = Groupe : { $name } ({ $group })
recovery-person = Personne : { $name }
recovery-backups = Machines de sauvegarde :
recovery-secret = Gardez cette fiche comme un passeport : quiconque la détient peut ajouter des ordinateurs en votre nom. Imprimez-la et rangez-la en lieu sûr ; ne l’envoyez à personne.
recovery-instructions = Sur un nouvel ordinateur, enregistrez le code dans un fichier texte et lancez : pigeon recover CE-FICHIER DOSSIER
recovery-qr-too-large = Cette fiche contient trop d’informations pour un seul code QR. Gardez le code texte complet sur cette fiche ; il fonctionne de la même manière.
recovery-recommendation = { $name } n’a qu’un seul ordinateur pouvant signer. Imprimez une fiche de récupération ou ajoutez un second ordinateur pour pouvoir revenir si vous perdez cet ordinateur.
web-print-recovery = Préparer une fiche de récupération
web-recovery-print-help = Utilisez la commande Imprimer de votre navigateur pour imprimer cette page. Gardez le code texte complet avec le code QR. Fermez cette page une fois la fiche rangée en lieu sûr.

recovery-refresh-recommendation = Les machines de sauvegarde ont changé depuis la préparation des fiches de récupération de { $name }. Préparez une nouvelle fiche avec les machines actuelles.

problem-recovery-needs-backup = Ajoutez une machine de sauvegarde avant de préparer une fiche de récupération. Une sauvegarde doit recevoir l’autorisation de la fiche pour pouvoir vous aider à revenir.

problem-join-incomplete = Le nouveau groupe a été vérifié, mais la récupération n’a pas pu se terminer. Redémarrez pigeon pour terminer l’opération enregistrée. Votre ancien historique est conservé.
problem-repository-busy = Ce dépôt contient une mise à jour de fichier inachevée. Laissez pigeon la terminer, ou redémarrez pigeon avant de réessayer la récupération.
problem-folder-not-empty = Ce dossier n’est pas vide et n’appartient pas au groupe à récupérer. Choisissez un dossier vide ou le dépôt existant du groupe refondé.
