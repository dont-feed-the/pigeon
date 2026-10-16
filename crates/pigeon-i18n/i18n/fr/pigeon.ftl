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
headline-inventory-unreadable = Pigeon ne peut pas vérifier toutes les versions enregistrées pour le moment.
headline-too-few-backups = Pas encore en sécurité : ajoutez un ordinateur de sauvegarde.
why-capturing = Cette modification attend d'être enregistrée et copiée sur les ordinateurs de sauvegarde.
why-too-few-backups = { $have } ordinateurs de sauvegarde sur { $need } sont configurés. Pour en ajouter un, lancez dessus la ligne qu'affiche pigeon invite, avec --backup-machine à la fin.
headline-not-safe =
    { $files ->
        [one] { $files } version de fichier n'est pas encore en sécurité.
       *[other] { $files } versions de fichiers ne sont pas encore en sécurité.
    }
headline-working =
    { $files ->
        [one] Envoi de { $files } version de fichier aux machines de sauvegarde…
       *[other] Envoi de { $files } versions de fichiers aux machines de sauvegarde…
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
why-machine-not-heard = Cet ordinateur n’a pas encore eu de nouvelles de { $machine }.
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
cmd-invite = Afficher la ligne qui ajoute une personne, un ordinateur ou une machine de sauvegarde au groupe
cmd-join = Rejoindre un groupe avec son invitation
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
cmd-update = Installer la dernière version de pigeon, une version donnée, main compilé depuis les sources, ou la précédente ; ou activer ou désactiver la mise à jour automatique
cmd-completions = Afficher le script qui active la complétion avec Tab dans le shell
opt-json = Afficher le résultat en JSON, pour les scripts
opt-backup-machine = Pour une machine de sauvegarde plutôt que l'ordinateur d'une personne
opt-not-shared = Lister ce qui n'est pas partagé, et pourquoi
help-usage = Usage : pigeon COMMANDE [ARGUMENTS]
help-commands = Commandes :

## Résultats

free-result = Fichiers en sécurité qui peuvent être libérés : { $safe } sur { $total }. pigeon libérera leur espace en arrière-plan ; les autres fichiers attendent des copies sûres.
free-waiting =
    { $files ->
        [one] { $files } fichier n'est pas encore en sécurité ; pigeon le libérera dès qu'il le sera.
       *[other] { $files } fichiers ne sont pas encore en sécurité ; pigeon les libérera dès qu'ils le seront.
    }
keep-result = { $folder } sera gardé sur cet ordinateur. Les fichiers absents seront téléchargés dès qu’une copie sera joignable ; consultez Accueil pour suivre l’avancement.
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
restore-result = { $path } est restauré sur cet ordinateur ; la version plus récente reste dans son historique. Consultez Accueil pour le partage et la protection, et Suggestions si une approbation est nécessaire.
restore-folder-result = { $path } : { $files } fichiers restaurés sur cet ordinateur. Les fichiers plus récents et l’historique sont conservés. Consultez Accueil et Suggestions pour le partage et l’approbation.
problem-restore-folder-incomplete = { $path } : { $files } fichiers restaurés ; { $skipped } n’ont pas pu être restaurés et sont restés inchangés. L’opération nécessite votre attention. Fichiers à vérifier : { $paths }. Fermez les fichiers utilisés et réessayez ; lancez pigeon doctor pour les autres problèmes. Consultez Accueil et Suggestions pour le partage et l’approbation.
problem-restore-in-use = { $path } a changé ou est en cours d’utilisation. Pigeon l’a laissé inchangé. Ouvrez une ancienne version avec « Ouvrir une copie », ou fermez le fichier et réessayez.
problem-restore-folder-version = Pour restaurer un dossier, choisissez une date avec --at ou omettez l’option de version pour restaurer son dernier état enregistré.
web-restore-folder = Restaurer ce dossier
web-files-arriving = En attente de { $files } fichiers pour qu’ils soient disponibles sur cet ordinateur. Laissez pigeon fonctionner ; il les recevra dès qu’une copie sera joignable.
web-restore-folder-help = Restaurez un dossier entièrement supprimé dans son dernier état enregistré. Les fichiers plus récents et l’historique sont conservés. Les fichiers individuels figurent ci-dessous.
recovery-sheet-text = { $sheet }
recovered = Cet ordinateur est de retour dans { $group }. Ses fichiers arrivent depuis les machines de sauvegarde ; retirez ensuite les ordinateurs perdus, révoquez la fiche utilisée avec remove-computer et imprimez une nouvelle fiche.
not-shared-kind = { $path } : non partagé ({ $kind })
doctor-lab-build = Ceci est une version de laboratoire : ses rapports d'état sont marqués, et aucune version publiée ne les compte.
doctor-version = { $program } { $found } (il faut au moins { $minimum })
doctor-ok = Tout ce dont pigeon a besoin est en place.
web-doctor = Vérifier l’installation
problem-transport-helper-unavailable = Le programme de transport de pigeon manque, ne fonctionne pas ou appartient à une autre version.
hint-transport-helper-unavailable = Réinstallez cette version de pigeon avec tous ses programmes, puis redémarrez pigeon et relancez pigeon doctor.
created =
    Le groupe « { $group } » est créé. Pour ajouter quelqu'un, un autre ordinateur ou une machine de sauvegarde, partagez cette ligne ; elle installe pigeon et rejoint le groupe (sur une machine de sauvegarde, ajoutez --backup-machine à la fin). Quiconque l'a peut rejoindre, avec les droits de la personne qu'il dit être :
    { $command }
invitation =
    Pour ajouter quelqu'un à « { $group } », un autre ordinateur ou une machine de sauvegarde, partagez cette ligne ; elle installe pigeon et rejoint le groupe (sur une machine de sauvegarde, ajoutez --backup-machine à la fin). Quiconque l'a peut rejoindre, avec les droits de la personne qu'il dit être :
    { $command }
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
web-join-refounded = { $group } a été refondé par { $name } le { $date }. Le rejoindre, c'est accepter { $name } comme administrateur. Cliquez sur Rejoindre pour continuer.
confirm-held-share = Supprimer les fichiers de { $batch } pour tout le monde ? Ils restent dans l'historique. [o/N]

## Problèmes (10 §10.6, ADR 0023) : problem-<code>, et hint-<code> s'il y en a un

problem-code = Code : { $code }
problem-not-in-this-build = « { $command } » n'est pas encore dans cette version de pigeon.
hint-not-in-this-build = Cette première version n'a que les commandes qui mettent les fichiers en sécurité ; les autres viendront.
problem-file-system-not-in-this-build = Ce dossier est sur { $fs }, que cette version de pigeon ne prend pas encore en charge.
hint-file-system-not-in-this-build = Choisissez un dossier sur ext4 (Linux) ou APFS (macOS).
problem-invite-unknown = Le groupe ne connaît pas cette invitation.
problem-invite-not-allowed = { $inviter } ne peut pas ajouter cet ordinateur au groupe.
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
web-override-reason = Raison de la dérogation administrateur
web-managed-client = Voici le pair Pigeon de votre appareil sur la passerelle. Choisissez les fichiers à garder hors ligne dans votre application Nextcloud.
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
web-invitation-code = La ligne à partager
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
left = { $name } a quitté le groupe « { $group } », dont il était la seule machine. Ses fichiers restent dans le dossier.
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
files-none = Ce dossier n'a encore aucun fichier.
file-row = { $path } : { $state }
web-restore = Restaurer cette version
web-view = Ouvrir une copie
web-versions = Versions
deleted-none = Aucun fichier supprimé à récupérer.
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
problem-join-history-unverified = Pigeon n’a pas pu vérifier l’historique de ce groupe. Demandez à la personne qui vous a invité de mettre pigeon à jour, puis réessayez.
problem-export-history-unverified = Pigeon n’a pas pu vérifier l’historique archivé. Conservez ce dépôt et demandez une copie complète à un autre membre avant d’exporter les fichiers supprimés.

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
problem-folder-not-empty = Ce dossier contient déjà des fichiers. Choisissez un dossier vide ou nouveau ; pour rejoindre un groupe refondé, choisissez le dossier de son dépôt existant.
# Group health from replicated state (ADR 0025).
web-health = État du groupe
web-health-current = Fichiers actuels
web-health-retained = Autres versions conservées
web-health-unavailable = Pas encore vérifié ; pigeon est peut-être occupé.
web-health-no-report = Aucun état reçu pour le moment.
web-health-counts = En sécurité : { $protected } · Partagés, pas encore en sécurité : { $published } · Seulement sur cet ordinateur : { $local } · Pas vérifiés récemment : { $uncertain } · Manquants : { $missing } · Effacés définitivement : { $erased }
headline-empty = Aucun fichier pour le moment. Ajoutez des fichiers dans votre dossier partagé pour commencer.
web-health-size = { $mb } Mo
web-health-kb = { $kb } ko
web-health-bytes = { $bytes } o
web-health-no-limit = non définie
web-health-paper = Une clé papier ne conserve aucun fichier.
web-health-disk = Conservés : { $used } · Libres : { $free } · Limite : { $limit } · Réserve : { $reserve }
web-health-full = Ce disque a besoin de plus d’espace libre.
web-health-lost = Cette machine de secours signale la perte de son dépôt.
web-health-checking = Vérification des fichiers conservés ; ces copies ne comptent pas encore pour la protection.
web-health-queue =
    { $deferred ->
        [one] À envoyer : { $send } · À recevoir : { $receive } · En attente d’espace : { $deferred } élément, { $missing } nécessaires
       *[other] À envoyer : { $send } · À recevoir : { $receive } · En attente d’espace : { $deferred } éléments, { $missing } nécessaires
    }
web-health-report = État du stockage et des tâches : { $when }.
web-health-report-old = État du stockage et des tâches ancien : { $when }. Un nouveau contact ne met pas ces chiffres à jour.
web-health-suggestions = Suggestions en attente d’une décision : { $count }
web-health-repair = Contenus marqués à recopier : { $count }
web-health-updated = Vue mise à jour : { $when }. Les chiffres de stockage portent leur propre date.
web-health-omitted = { $count } autres explications de fichiers ne sont pas affichées ici. Choisissez un dossier dans Fichiers.
web-health-machines = Ordinateurs, contacts et stockage
web-health-epochs = Après le retrait d’ordinateurs
web-health-no-epochs = Aucun ordinateur conservant des fichiers n’a été retiré.
web-health-epoch = { $name } a été retiré : { $when }.

cmd-folder-rule = Voir les règles de partage, ou laisser un administrateur choisir le propriétaire d’un dossier ou qui décide des suggestions. --owner me désigne votre dossier.
folder-rule-owned = { $folder } : seul { $owner } peut modifier ce dossier.
folder-rule-add-only = { $folder } : tout le monde peut ajouter ; les autres modifications sont décidées par { $stewards }.
folder-rule-result = La règle de partage de { $folder } est enregistrée sur cet ordinateur. Les autres l’appliqueront après leur prochaine vérification du partage. Consultez la règle du dossier sur un autre ordinateur pour vérifier son arrivée.
problem-folder-rule-path = Choisissez un sous-dossier au nom portable, avec son orthographe d’origine. La racine du groupe et les fichiers propres à pigeon ne peuvent pas recevoir de règle.
problem-folder-rule-person = Aucun membre actuel unique ne correspond à « { $name } ». Choisissez son nom dans Personnes et machines, ou utilisez me pour vous-même.
web-folder-rule = Règle de partage du dossier
web-subfolders = Sous-dossiers
web-folder-rule-help = Choisissez qui peut modifier ce dossier. Les autres peuvent suggérer des modifications. Les sous-dossiers héritent de cette règle sauf s’ils ont la leur.
web-folder-rule-admin = Un administrateur peut modifier cette règle de partage.
web-folder-rule-owner = Une seule personne peut le modifier
web-folder-rule-add-only = Tout le monde peut ajouter ; les personnes choisies décident des autres modifications
web-folder-rule-inherit = Hériter de la règle du dossier parent
web-folder-rule-stewards = Personnes qui décident des modifications
web-folder-rule-default-admins = Ne cochez personne pour laisser les administrateurs décider.

web-view-suggestion = Examiner le fichier suggéré
suggestion-deletes-file = Cette suggestion supprime { $path }. Le fichier partagé reste inchangé tant que la suggestion n’est pas acceptée.

space-title = Espace de sauvegarde
space-need =
    { $copies ->
        [one] Stockage nécessaire pour { $copies } copie de sauvegarde : fichiers actuels { $current } octets ; historique { $history } octets ; suggestions { $suggestions } octets.
       *[other] Stockage nécessaire pour { $copies } copies de sauvegarde : fichiers actuels { $current } octets ; historique { $history } octets ; suggestions { $suggestions } octets.
    }
space-offered = Capacité configurée pour ce contenu : { $bytes } octets. Chaque ordinateur de sauvegarde compte pour une copie au maximum, même sans état de stockage récent.
space-overflow = Copies supplémentaires : { $bytes } octets, selon les emplacements signalés.
space-overflow-unknown = Les copies supplémentaires n’ont pas été vérifiées.
space-short = La capacité de sauvegarde configurée a moins de 25 % de marge. Ajoutez un autre ordinateur de sauvegarde, ou augmentez une limite inférieure à une copie. L’espace disque libre signalé par chaque ordinateur est affiché ci-dessous.
space-disk = Stockage utilisé { $used } octets ; libre { $free } octets ; limite { $limit } ; réserve { $reserve } octets.
space-proposal = Suggestion { $id }, { $state }, enregistrée { $when } : { $bytes } octets supplémentaires avant les copies de sauvegarde.
space-shared = Un contenu commun est compté une seule fois dans les totaux et peut apparaître dans plusieurs suggestions. Les suggestions refusées restent jusqu’à leur effacement.
space-open = en attente d’une décision
space-adopted = acceptée
space-rejected = refusée
space-withdrawn = retirée
space-snapshot = { $group }, vérifié le { $when }.

web-health-suggestions-unavailable = Suggestions en attente d’une décision : pas encore vérifiées.
web-health-separator = {" : "}
web-not-found = Il n’y a pas de page ici.
web-noscript = Cette page ne se met à jour d’elle-même qu’avec JavaScript. Rechargez-la pour voir l’état le plus récent.
problem-folder-not-absolute = « { $folder } » n’est pas un chemin de dossier complet. Indiquez-en un qui commence par /, par exemple /home/vous/Famille.
problem-name-empty = Donnez un nom au groupe et à cet ordinateur.
problem-copies-zero = Un groupe a besoin d’au moins une copie de sauvegarde.

ask-join-folder = Dossier pour « { $group } » [{ $folder }] :
ask-join-person = Votre nom dans le groupe [{ $person }] :
confirm-join-existing = { $person } fait déjà partie de « { $group } ». Ajouter cet ordinateur à { $person }, avec les mêmes droits ? [o/N]
problem-person-exists = { $person } fait déjà partie du groupe.
problem-already-in-group = Cet ordinateur fait déjà partie de ce groupe.
problem-in-another-group = Cet ordinateur fait déjà partie du groupe « { $group } », et un ordinateur ne peut être que dans un seul groupe.
hint-in-another-group = Pour le quitter, lancez pigeon leave, ou choisissez Quitter le groupe dans Réglages dans pigeon open ; ses fichiers restent dans son dossier. Puis réessayez.
leaving-wait = Il est en train de quitter ce groupe : réessayez dès qu'une autre machine du groupe a son retrait.
web-your-name = Votre nom dans le groupe
web-join-existing = { $person } fait déjà partie de « { $group } ». Cliquez sur Rejoindre pour ajouter cet ordinateur à { $person }, avec les mêmes droits, ou revenez en arrière pour donner un autre nom.
web-copy = Copier
web-invite-help = Pour ajouter quelqu'un à « { $group } », un autre ordinateur ou une machine de sauvegarde, partagez cette ligne ; elle installe pigeon et rejoint le groupe. Sur une machine de sauvegarde, ajoutez --backup-machine à la fin. Quiconque l'a peut rejoindre, avec les droits de la personne qu'il dit être.
progress-join = Adhésion : en attente de la réponse de l'ordinateur d'un administrateur du groupe…
