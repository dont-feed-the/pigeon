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
cmd-update = Installer la dernière version de pigeon, une version donnée, ou la précédente
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
recovered = Cet ordinateur est de retour dans { $group }. Ses fichiers arrivent depuis les machines de sauvegarde ; retirez ensuite les ordinateurs perdus et imprimez une nouvelle fiche de récupération.
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
machine-this = { $name } : cet ordinateur
version-row = { $number }. { $saved } : { $state }
waiting-row = { $path } : { $state }
confirm-remove-computer = Retirer { $name } du groupe ? Ses copies ne compteront plus. [o/N]
confirm-yes = oui

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
hint-daemon-not-running = Démarrez-le, puis vérifiez avec : pigeon doctor
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
web-pause = Mettre en pause
web-resume = Reprendre le partage
pause-hour = pendant une heure
pause-day = pendant un jour
pause-forever = jusqu'à ce que je reprenne
problem-pause-duration = « { $duration } » n'est pas une durée que pigeon comprend.
hint-pause-duration = Écrivez des minutes, des heures ou des jours, par exemple 45m, 2h ou 3d.
web-remove = Retirer
web-remove-confirm = Retirer { $name } du groupe ? Ses copies ne compteront plus.
web-invite = Inviter
web-invite-person = Le nom de la personne
web-invite-admin = comme admin
web-invite-backup = Inviter une machine de sauvegarde
files-none = Ce dossier n'a encore aucun fichier.
file-row = { $path } : { $state }
web-restore = Restaurer cette version
web-versions = Versions
deleted-none = Aucun fichier n'a été supprimé.
deleted-row = { $path }, supprimé { $deleted }
web-deleted = Fichiers supprimés
web-restore-deleted = Restaurer
notify-needs-you = pigeon a besoin de vous : ouvrez-le pour voir quoi faire.
notify-not-safe-long = Des fichiers ne sont pas en sécurité depuis plus d'un jour.
notify-waiting = Une suggestion ou un conflit vous attend.
notify-open = Ouvrir pigeon
