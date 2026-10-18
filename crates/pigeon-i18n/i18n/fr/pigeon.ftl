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
state-not-here = { $state }, pas sur cet ordinateur

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
headline-history-waiting = La copie des fichiers attend la vérification des versions enregistrées. Si cela dure, lancez : pigeon doctor
headline-too-few-backups = Pas encore en sécurité : ajoutez un ordinateur de sauvegarde.
why-capturing = pigeon n'a pas encore enregistré cette modification ni créé ses copies sur les ordinateurs de sauvegarde.
why-differs = Votre copie diffère de la version du groupe. Choisissez laquelle garder.
why-name = Son nom ne fonctionne pas sur tous les ordinateurs. Renommez-le.
why-held-deletion = Il a été supprimé avec beaucoup d’autres fichiers. pigeon attend votre choix : les supprimer pour tout le monde, ou les faire revenir.
why-capture-failed =
    pigeon n’a pas pu enregistrer cette modification : { $why ->
        [disk-full] le disque est plein
        [no-access] il n’a pas le droit de lire le fichier
        [busy] un autre programme utilisait le dossier
       *[other] un problème est survenu
    }. Il réessaiera.
why-removed = Cet ordinateur ne fait plus partie du groupe : ce fichier n’est donc plus sauvegardé.
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
headline-removed = Cet ordinateur a été retiré du groupe. Ses fichiers restent dans son dossier, mais ils ne sont plus partagés ni sauvegardés. Pour les partager à nouveau, demandez à quelqu’un du groupe sa ligne d’invitation et rejoignez-le à nouveau.
why-machine-offline = { $machine } a donné des nouvelles pour la dernière fois { $since }.
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

## Temps à venir, à l’heure locale de cet ordinateur, et tailles (units.rs)

time-clock = { $hour } h { $minute }
time-month =
    { $index ->
        [1] janvier
        [2] février
        [3] mars
        [4] avril
        [5] mai
        [6] juin
        [7] juillet
        [8] août
        [9] septembre
        [10] octobre
        [11] novembre
       *[12] décembre
    }
time-date =
    { $day ->
        [1] 1er { $month }
       *[other] { $day } { $month }
    }
time-date-year =
    { $day ->
        [1] 1er { $month } { $year }
       *[other] { $day } { $month } { $year }
    }
time-until =
    { $moment ->
        [today] jusqu’à { $clock }
        [tomorrow] jusqu’à demain à { $clock }
       *[other] jusqu’au { $date } à { $clock }
    }
time-expires =
    { $moment ->
        [today] expire à { $clock }
        [tomorrow] expire demain à { $clock }
       *[other] expire le { $date } à { $clock }
    }
number-decimal = { $whole },{ $tenth }
number-thousands = { $high } { $low }
size-bytes =
    { $plural ->
        [one] { $size } octet
       *[other] { $size } octets
    }
size-kb = { $size } ko
size-mb = { $size } Mo
size-gb = { $size } Go
size-tb = { $size } To

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
cmd-sync = Voir quels dossiers sont sur cet ordinateur ; --default rend à un dossier son choix par défaut
cmd-suggestions = Lister les suggestions qui attendent une décision
cmd-accept = Accepter une suggestion dans les fichiers du groupe ; nommez des fichiers pour n’accepter qu’eux et refuser les autres
cmd-decline = Refuser une suggestion, en disant pourquoi si vous voulez ; nommez des fichiers pour ne refuser qu’eux et accepter les autres
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
free-preview-leave =
    { $files ->
        [one] { $files } fichier sera retiré de cet ordinateur.
       *[other] Ces { $files } fichiers seront retirés de cet ordinateur.
    }
free-preview-stay =
    { $files ->
        [one] Il reste en sécurité sur les machines de sauvegarde : { $machines }.
       *[other] Ils restent en sécurité sur les machines de sauvegarde : { $machines }.
    }
free-preview-none = Aucun fichier ici n’est encore en sécurité sur les machines de sauvegarde ; rien ne quitte donc cet ordinateur pour l’instant.
free-preview-undo = « Garder sur cet ordinateur » les ramène quand vous voulez.
problem-backups-quiet =
    { $backups ->
        [one] La machine de sauvegarde { $machines } a donné des nouvelles pour la dernière fois { $heard } ; pigeon ne peut donc pas confirmer pour l’instant qu’elle a toujours ces fichiers.
       *[other] Les machines de sauvegarde ({ $machines }) ont donné des nouvelles pour la dernière fois { $heard } ; pigeon ne peut donc pas confirmer pour l’instant qu’elles ont toujours ces fichiers.
    }
free-backups-never =
    { $backups ->
        [one] pigeon n’a encore jamais eu de nouvelles de la machine de sauvegarde { $machines } ; il ne peut donc pas confirmer qu’elle a ces fichiers.
       *[other] pigeon n’a encore jamais eu de nouvelles des machines de sauvegarde ({ $machines }) ; il ne peut donc pas confirmer qu’elles ont ces fichiers.
    }
hint-backups-quiet = Si vous continuez, les fichiers quitteront cet ordinateur dès que pigeon aura des nouvelles des machines de sauvegarde et aura confirmé leurs copies, même en votre absence.
confirm-free-quiet = Libérer l’espace quand même ? [o/N]
web-free-title = Libérer l’espace de { $folder } ?
web-free-quiet-title = Les machines de sauvegarde n’ont pas donné de nouvelles
web-free-anyway = Libérer l’espace quand même
web-free-cancel = Annuler
keep-result = { $folder } sera gardé sur cet ordinateur. Les fichiers absents seront téléchargés dès qu’une copie sera joignable ; consultez Accueil pour suivre l’avancement.
suggestions-none = Aucune suggestion n'attend.
suggestion-decide = { $id }  { $author } suggère une modification de { $paths }, enregistrée { $saved }. Acceptez-la avec : pigeon accept { $id }
suggestion-other = { $id }  { $author } suggère une modification de { $paths }, enregistrée { $saved } ; quelqu'un d'autre décide.
suggestion-yours = { $id }  Votre modification de { $paths }, enregistrée { $saved }, attend une décision.
accepted-suggestion = La suggestion de { $author } est maintenant dans les fichiers du groupe.
declined-suggestion = La suggestion de { $author } est refusée ; la personne sera prévenue.
decided-all-accepted =
    { $count ->
        [one] Vous avez accepté la modification de { $author } ({ $paths }). Nous préviendrons { $author }.
       *[other] Vous avez accepté les { $count } modifications de { $author } ({ $paths }). Nous préviendrons { $author }.
    }
decided-all-declined =
    { $count ->
        [one] Vous avez refusé la modification de { $author } ({ $paths }). Nous préviendrons { $author }.
       *[other] Vous avez refusé les { $count } modifications de { $author } ({ $paths }). Nous préviendrons { $author }.
    }
decided-some = Vous avez accepté { $accepted } des modifications de { $author } ({ $accepted-paths }) et refusé { $declined } ({ $declined-paths }). Nous préviendrons { $author }.
suggestion-file-added = { $path } : nouveau fichier, { $after }
suggestion-file-changed = { $path } : modifié, { $before } → { $after }
suggestion-file-changed-lines =
    { $path } : modifié, { $before } → { $after }, { $added ->
        [one] { $added } ligne ajoutée
       *[other] { $added } lignes ajoutées
    } et { $removed ->
        [one] { $removed } retirée
       *[other] { $removed } retirées
    }
suggestion-file-deleted = { $path } : supprimé (il faisait { $before })
suggestion-file-renamed = { $path } : renommé, il s’appelait { $from } ; contenu inchangé ({ $after })
suggestion-files-hint = Pour n’accepter que certains fichiers et refuser les autres : pigeon accept { $id } FICHIER…
suggestion-outcome-accepted =
    { $count ->
        [one] { $by } a accepté votre modification ({ $paths }), { $when }.
       *[other] { $by } a accepté vos { $count } modifications ({ $paths }), { $when }.
    }
suggestion-outcome-declined =
    { $count ->
        [one] { $by } a refusé votre modification ({ $paths }), { $when }.
       *[other] { $by } a refusé vos { $count } modifications ({ $paths }), { $when }.
    }
suggestion-outcome-some = { $by } a accepté { $accepted } de vos modifications ({ $accepted-paths }) et en a refusé { $declined } ({ $declined-paths }), { $when }.
suggestion-outcome-someone = Quelqu’un
suggestion-outcome-note = Note de { $by } : « { $note } »
suggestion-outcome-kept = Les fichiers refusés restent tels que vous les avez enregistrés sur cet ordinateur ; personne d’autre n’a votre version.
conflicts-none = Chaque fichier ici est la version du groupe.
conflict-waiting = { $path } : votre suggestion attend une décision.
conflict-choose = { $path } : votre version diffère de celle du groupe. Choisissez avec : pigeon resolve
resolved-take-group = { $path } est de nouveau la version du groupe ; la vôtre reste dans son historique.
resolved-keep-copy = Votre version de { $path } est gardée sous { $dest } ; { $path } est de nouveau la version du groupe.
resolved-suggest-again = Votre version de { $path } est suggérée à nouveau.
web-conflicts = Fichiers différents de la version du groupe
web-conflict-row = { $path } : votre copie ici est différente de la version du groupe. Ouvrez-la pour choisir ce que vous gardez.
web-conflict-row-waiting = { $path } : votre copie ici est différente de la version du groupe ; votre modification attend une décision.
web-conflict-heading = Votre copie est différente
web-conflict-explain = Votre copie de { $name } est différente de la version du groupe. Choisissez quoi faire ; rien n’est perdu.
web-conflict-waiting-explain = Votre copie de { $name } est différente de la version du groupe : votre modification a été suggérée et attend une décision. Vous pouvez attendre ou choisir maintenant ; rien n’est perdu.
web-resolve-take-group = Prendre la version du groupe
web-resolve-take-group-help = Ce fichier devient la version du groupe. La vôtre n’est pas supprimée : pigeon la garde dans son historique, mais plus comme fichier dans votre dossier. Pour la garder comme fichier, choisissez « Garder la mienne en copie ».
web-resolve-take-group-waiting-help = Votre suggestion est retirée et ce fichier devient la version du groupe. pigeon garde la vôtre dans son historique, mais plus comme fichier dans votre dossier. Pour la garder comme fichier, choisissez « Garder la mienne en copie ».
web-resolve-keep-copy = Garder la mienne en copie
web-resolve-keep-copy-help = Votre version est enregistrée dans un nouveau fichier, sous le nom ci-dessous, à côté de celui-ci, et partagée avec le groupe. Ce fichier devient la version du groupe.
web-resolve-copy-name = Nom de votre copie
web-resolve-copy-suffix = ma copie
web-resolve-suggest-again = Suggérer à nouveau
web-resolve-suggest-again-help = Votre version reste dans ce fichier et est de nouveau suggérée au groupe. La version du groupe reste telle quelle jusqu’à ce que votre suggestion soit acceptée.
undo-free = Annuler : pigeon keep { $folder }
undo-keep = Annuler : pigeon free { $folder }
restore-result = { $path } est restauré sur cet ordinateur. Restaurer un ancien contenu crée une nouvelle version actuelle et conserve l’historique existant. Les demandes d’effacement antérieures restent en attente mais ne couvrent pas cette nouvelle version. Consultez Accueil pour le partage et la protection, et Suggestions si une approbation est nécessaire.
restore-folder-result = { $path } : { $files } fichiers restaurés sur cet ordinateur. Les fichiers plus récents et l’historique sont conservés. Consultez Accueil et Suggestions pour le partage et l’approbation.
problem-restore-folder-incomplete = { $path } : { $files } fichiers restaurés ; { $skipped } n’ont pas pu être restaurés et sont restés inchangés. L’opération nécessite votre attention. Fichiers à vérifier : { $paths }. Fermez les fichiers utilisés et réessayez ; lancez pigeon doctor pour les autres problèmes. Consultez Accueil et Suggestions pour le partage et l’approbation.
problem-restore-in-use = { $path } a changé ou est en cours d’utilisation. Pigeon l’a laissé inchangé. Ouvrez une ancienne version avec « Ouvrir une copie », ou fermez le fichier et réessayez.
problem-restore-waiting = La restauration de { $path } a été demandée, mais son contenu enregistré n’est pas disponible sur cet ordinateur. Les modifications locales sont conservées. Consultez Accueil pour les copies disponibles, et Suggestions si une approbation est nécessaire. Réessayez la restauration lorsqu’une copie sera disponible.
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
joined-renamed =
    { $who ->
        [backup-machine] Cette machine de sauvegarde garde maintenant des copies pour « { $group } ».
       *[computer] Cet ordinateur fait maintenant partie de « { $group } ». Les fichiers du groupe arrivent dans son dossier.
    } Un autre ordinateur du groupe s’appelait déjà { $asked } : celui-ci s’appelle donc { $name }.
removed-computer = { $name } est retiré du groupe ; ses copies ne comptent plus.
removed-from-group = Cet ordinateur ne fait plus partie du groupe : voici le groupe tel qu’il le connaissait. Pour partager à nouveau des fichiers, demandez à quelqu’un du groupe sa ligne d’invitation et rejoignez-le à nouveau.
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
confirm-remove-computer = Retirer { $name } du groupe ? Ses fichiers y restent, mais ne sont plus partagés avec le groupe, et ses copies ne comptent plus. Le retrait ne change pas la ligne d’invitation : une personne qui la possède peut rejoindre comme nouvel ordinateur. [o/N]
confirm-remove-backup = Retirer la machine de sauvegarde { $name } du groupe ? Les copies qu’elle garde ne comptent plus ; les autres machines de sauvegarde prennent le relais. Le retrait ne change pas la ligne d’invitation : une personne qui la possède peut rejoindre comme nouvel ordinateur. [o/N]
confirm-remove-backup-unsafe = Retirer la machine de sauvegarde { $name } du groupe ? { $left ->
        [0] Après cela, le groupe n’aura plus aucune machine de sauvegarde. Aucun de vos fichiers ne sera en sécurité tant que vous n’en aurez pas ajouté une.
        [one] Après cela, le groupe n’aura plus qu’une machine de sauvegarde, alors qu’il lui en faut { $needed } pour que vos fichiers soient en sécurité. Ils ne le seront pas tant que vous n’en aurez pas ajouté une autre.
       *[other] Après cela, le groupe n’aura plus que { $left } machines de sauvegarde, alors qu’il lui en faut { $needed } pour que vos fichiers soient en sécurité. Ils ne le seront pas tant que vous n’en aurez pas ajouté une autre.
    } Le retrait ne change pas la ligne d’invitation : une personne qui la possède peut rejoindre comme nouvel ordinateur. [o/N]
confirm-yes = oui
confirm-leave = Retirer cet ordinateur du groupe ? Ses fichiers restent dans le dossier, mais il cesse de les partager. [o/N]
confirm-refound = Refonder le groupe avec vous pour seul administrateur ? Il devient un nouveau groupe : chaque autre membre le rejoint à nouveau par une invitation de votre part, et l'ancien historique reste lisible. Les effacements achevés dans cet état restent effectifs. Les demandes en attente ne continuent pas dans le nouveau groupe ; les ordinateurs restés dans l’ancien peuvent les terminer là-bas. Préparez à nouveau l’effacement après avoir réinvité vos sauvegardes. [o/N]
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
problem-join-version-unsupported = Ces ordinateurs utilisent des versions différentes pour rejoindre un groupe. Mettez pigeon à jour sur les deux, puis réutilisez la ligne d’invitation du groupe.
problem-invite-not-allowed = { $inviter } ne peut pas ajouter cet ordinateur au groupe.
problem-inviter-offline = L'ordinateur de { $inviter } est injoignable pour le moment.
hint-inviter-offline = Réessayez quand l'ordinateur de { $inviter } est allumé. Rien n’a été gardé sur cet ordinateur : vous pouvez reprendre le même dossier.
problem-not-in-group = Cet ordinateur n'est pas membre du groupe.
problem-not-allowed = Vous n'avez pas le droit de faire cela dans ce groupe.
problem-suggestion-unknown = Aucune suggestion en attente ne s'appelle { $id }.
hint-suggestion-unknown = Voyez-les avec : pigeon suggestions
problem-suggestion-file-unknown = { $path } ne fait pas partie des fichiers de cette suggestion.
hint-suggestion-file-unknown = Voyez ses fichiers avec : pigeon suggestions
problem-no-files-chosen = Choisissez au moins un fichier de la suggestion.
hint-no-files-chosen = Voyez ses fichiers avec : pigeon suggestions
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
problem-content-not-here = Le contenu de { $path } n'est pas sur cet ordinateur.
hint-content-not-here = Gardez son dossier sur cet ordinateur pour le lire ici, ou ouvrez une copie depuis sa page.
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
web-suggestion-label = la modification de { $paths } par { $author }
web-accept = Accepter
web-override-reason = Pourquoi vous l’acceptez quand même (obligatoire)
web-managed-client = Voici le pair Pigeon de votre appareil sur la passerelle. Choisissez les fichiers à garder hors ligne dans votre application Nextcloud.
web-decline = Refuser
web-decline-note = Pourquoi (facultatif)
web-suggestions-for-you = À vous de décider
web-suggestions-yours = Vos suggestions
web-suggestions-others = À quelqu’un d’autre de décider
web-suggestions-decided = Vos suggestions décidées récemment
web-suggestion-files = Fichiers de cette suggestion
web-suggestion-choose = Tous les fichiers sont sélectionnés. « Accepter la sélection » accepte les fichiers sélectionnés et refuse les autres ; « Refuser la sélection » refuse les fichiers sélectionnés et accepte les autres.
web-review-file = Examiner { $path }
web-accept-selected = Accepter la sélection
web-decline-selected = Refuser la sélection
web-note-for = Note pour { $author } sur les fichiers refusés (facultatif)
web-decide-all = { $count } suggestions vous attendent. Pour toutes les décider d’un coup, avec tous leurs fichiers :
web-accept-all = Tout accepter
web-decline-all = Tout refuser
web-decided-all =
    { $count ->
        [one] Vous avez décidé { $count } suggestion.
       *[other] Vous avez décidé { $count } suggestions.
    }
web-decide-all-skipped = La suggestion de { $author } modifie quelque chose que personne ne peut normalement modifier : un administrateur peut quand même l’accepter, en disant pourquoi. Décidez-la à part.
web-file-suggestion-waiting = Votre modification de ce fichier, enregistrée { $saved }, attend une décision. En attendant, les autres gardent la version du groupe.
web-back = Retour
web-form-expired = Cette page n'est plus à jour. Rechargez-la et réessayez.
web-nav-files = Fichiers
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
        [one] { $path } : { $state }, { $files } fichier, pas sur cet ordinateur
       *[other] { $path } : { $state }, { $files } fichiers, pas sur cet ordinateur
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
headline-paused = Le partage est en pause { $until }. Les changements ne sont enregistrés que sur cet ordinateur.
headline-paused-until-resume = Le partage est en pause. Les changements ne sont enregistrés que sur cet ordinateur jusqu'à la reprise.
paused-until = Le partage est en pause { $until }.
paused-until-resume = Le partage est en pause jusqu'à ce que vous le repreniez.
resumed = Le partage reprend.
cmd-leave = Retirer cet ordinateur du groupe ; ses fichiers restent
leaving = { $name } quitte le groupe dès qu'une autre machine a son retrait. Ses fichiers restent dans le dossier.
leaving-already = Cet ordinateur quitte déjà le groupe.
left = { $name } a quitté le groupe « { $group } », dont il était la seule machine. Ses fichiers restent dans le dossier.
left-unreadable = Cet ordinateur a quitté le groupe « { $group } », créé par une version antérieure de pigeon que celle-ci ne sait pas lire. Ses fichiers restent dans le dossier.
cmd-refound = Refonder le groupe avec vous pour seul administrateur, quand aucun administrateur ne peut signer
refounded = { $group } est refondé avec { $name } pour seul administrateur. Invitez à nouveau les autres : leurs dossiers le rejoignent sur place.
cmd-held = Changements que pigeon retient jusqu'à votre choix : suppressions à partager ou annuler, noms à renommer
held-none = Rien n'est retenu.
held-row =
    { $files ->
        [one] { $id } : { $reason } ({ $files } fichier, depuis { $since }) : { $paths }
       *[other] { $id } : { $reason } ({ $files } fichiers, depuis { $since }) : { $paths }
    }
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
web-remove-confirm = Retirer { $name } du groupe ? Ses fichiers y restent, mais ne sont plus partagés avec le groupe, et ses copies ne comptent plus. Le retrait ne change pas la ligne d’invitation : une personne qui la possède peut rejoindre comme nouvel ordinateur.
web-remove-backup-confirm = Retirer la machine de sauvegarde { $name } du groupe ? Les copies qu’elle garde ne comptent plus. Le retrait ne change pas la ligne d’invitation : une personne qui la possède peut rejoindre comme nouvel ordinateur.
remove-backup-unsafe =
    { $left ->
        [0] Après cela, le groupe n’aura plus aucune machine de sauvegarde. Aucun de vos fichiers ne sera en sécurité tant que vous n’en aurez pas ajouté une.
        [one] Après cela, le groupe n’aura plus qu’une machine de sauvegarde, alors qu’il lui en faut { $needed } pour que vos fichiers soient en sécurité. Ils ne le seront pas tant que vous n’en aurez pas ajouté une autre.
       *[other] Après cela, le groupe n’aura plus que { $left } machines de sauvegarde, alors qu’il lui en faut { $needed } pour que vos fichiers soient en sécurité. Ils ne le seront pas tant que vous n’en aurez pas ajouté une autre.
    }
web-invite = Inviter
files-none = Ce dossier n'a encore aucun fichier.
sync-none = Le groupe n’a encore aucun dossier.
sync-kept = { $folder } : sur cet ordinateur
sync-not-kept = { $folder } : pas sur cet ordinateur
sync-chosen = (votre choix)
sync-others = (dossier de { $owner }, gardé seulement si vous le choisissez : pigeon keep)
sync-default-result = { $folder } suit de nouveau son choix par défaut sur cet ordinateur.
file-row = { $path } : { $state }
web-restore = Restaurer cette version
web-view = Télécharger une copie
web-versions = Versions
web-version-current = version actuelle
web-version-group-current = version actuelle du groupe
web-version-label = version { $number }, enregistrée { $saved }
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
web-held-share-confirm =
    { $files ->
        [one] Supprimer ce fichier pour tout le monde ({ $paths }) ? Il reste dans l’historique : on peut encore le faire revenir.
       *[other] Supprimer ces { $files } fichiers pour tout le monde ({ $paths }) ? Ils restent dans l’historique : on peut encore les faire revenir.
    }
web-held-row = { $reason }, { $since } : { $paths }
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
view-name-group = version du groupe
view-name-version = version { $number }
view-name-suggested = version suggérée

setting-telemetry-device = Partager les diagnostics avec
setting-off = Garder les diagnostics sur chaque ordinateur
problem-join-history-unverified = Pigeon n’a pas pu vérifier l’historique de ce groupe. Demandez à la personne qui vous a invité de mettre pigeon à jour, puis réessayez.
problem-join-files-unreadable = La préparation s’est arrêtée car certains fichiers du groupe n’ont pas pu être lus. Demandez de l’aide à un administrateur ; conservez ce dossier jusqu’à la résolution du problème.
problem-export-history-unverified = Pigeon n’a pas pu vérifier l’historique archivé. Conservez ce dépôt et demandez une copie complète à un autre membre avant d’exporter les fichiers supprimés.

problem-recovery-not-allowed = Cette fiche de récupération ne peut plus ajouter d’ordinateurs. Utilisez une fiche actuelle ou demandez une invitation à un administrateur.

machine-recovery = { $name } : fiche de récupération sur papier imprimée le { $date }
machine-recovery-undated = { $name } : fiche de récupération sur papier
web-revoke-sheet = Révoquer la fiche de récupération

revoked-sheet = { $name } est révoquée ; cette fiche de récupération ne peut plus ajouter d’ordinateurs.
web-revoke-confirm = Révoquer { $name } ? Cette fiche de récupération ne pourra plus ajouter d’ordinateurs.

progress-restore = Restauration de { $path } ; attente des vérifications du groupe et des copies disponibles.
progress-remove = Retrait de { $name } ; attente des vérifications du groupe.
progress-recovery-sheet = Préparation d’une fiche de récupération et attente de sa réception par une machine de sauvegarde.
progress-recover = Récupération de cet ordinateur ; vérification de l’historique de la sauvegarde et de l’autorisation du nouvel ordinateur.

confirm-revoke-sheet = Révoquer { $name } ? Cette fiche de récupération sur papier ne pourra plus ajouter d’ordinateurs. [o/N]

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
recovery-sheet-name = Cette fiche : { $name }, comme sur la page Membres et machines
recovery-backups = Machines de sauvegarde :
recovery-secret = Gardez cette fiche comme un passeport : quiconque la détient peut ajouter des ordinateurs en votre nom. Imprimez-la et rangez-la en lieu sûr ; ne l’envoyez à personne.
recovery-instructions = Sur un nouvel ordinateur, enregistrez le code dans un fichier texte et lancez : pigeon recover CE-FICHIER DOSSIER
recovery-qr-too-large = Cette fiche contient trop d’informations pour un seul code QR. Gardez le code texte complet sur cette fiche ; il fonctionne de la même manière.
recovery-recommendation = { $name } n’a qu’un seul ordinateur pouvant signer. Imprimez une fiche de récupération ou ajoutez un second ordinateur pour pouvoir revenir si vous perdez cet ordinateur.
web-print-recovery = Préparer une fiche de récupération
web-recovery-print-help = Utilisez la commande Imprimer de votre navigateur pour imprimer cette page. Gardez le code texte complet avec le code QR. Fermez cette page une fois la fiche rangée en lieu sûr.

recovery-refresh-recommendation = Les machines de sauvegarde ont changé depuis la préparation des fiches de récupération de { $name }. Préparez une nouvelle fiche avec les machines actuelles.

problem-recovery-needs-backup = Ajoutez une machine de sauvegarde avant de préparer une fiche de récupération. Une sauvegarde doit recevoir l’autorisation de la fiche pour pouvoir vous aider à revenir.
problem-recovery-backup-offline = Aucune machine de sauvegarde n’a répondu ({ $machines }). Une machine de sauvegarde doit recevoir la fiche avant son impression. Allumez-en une et connectez-la, puis préparez à nouveau la fiche. Rien n’a été modifié.
problem-recovery-not-received = Aucune machine de sauvegarde n’a reçu la fiche à temps : elle a donc été annulée et il n’y a rien à imprimer. Vérifiez qu’une machine de sauvegarde est allumée, puis préparez à nouveau la fiche.

problem-join-incomplete = Le nouveau groupe a été vérifié, mais la récupération n’a pas pu se terminer. Redémarrez pigeon pour terminer l’opération enregistrée. Votre ancien historique est conservé.
problem-repository-busy = Ce dépôt contient une mise à jour de fichier inachevée. Laissez pigeon la terminer, ou redémarrez pigeon avant de réessayer la récupération.
problem-folder-not-empty = Ce dossier contient déjà des fichiers. Choisissez un dossier vide ou nouveau ; pour rejoindre un groupe refondé, choisissez le dossier de son dépôt existant.
problem-file-system-unsupported = Le système de fichiers de ce dossier ne permet pas à pigeon de travailler en sécurité. Cette vérification ne modifie pas vos fichiers. Choisissez un dossier local sur ext4, btrfs ou xfs sous Linux, ou APFS sous macOS. Les machines de sauvegarde Linux peuvent aussi utiliser ZFS.
problem-file-system-unknown = pigeon n’a pas pu vérifier le système de fichiers de ce dossier. Vérifiez que le dossier ou son parent est accessible, puis réessayez.
# Group health from replicated state (ADR 0025).
web-health = État du groupe
web-health-current = Fichiers actuels
web-health-retained = Autres versions conservées
web-health-unavailable = Pas encore vérifié ; pigeon est peut-être occupé.
web-health-no-report = Aucun état reçu pour le moment.
web-health-counts = En sécurité : { $protected } · Partagés, pas encore en sécurité : { $published } · Seulement sur cet ordinateur : { $local } · Pas vérifiés récemment : { $uncertain } · Manquants : { $missing } · Effacés définitivement : { $erased }
headline-empty = Aucun fichier pour le moment. Ajoutez des fichiers dans votre dossier partagé pour commencer.
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

cmd-folder-rule = Voir les règles de partage, ou laisser un administrateur choisir le propriétaire d’un dossier ou qui décide des suggestions. --owner me désigne votre dossier, et --per-person un dossier où chacun possède le dossier à son nom.
folder-rule-owned = { $folder } : seul { $owner } peut modifier ce dossier.
folder-rule-add-only = { $folder } : tout le monde peut ajouter ; les autres modifications sont décidées par { $stewards }.
folder-rule-per-person = { $folder } : chaque dossier qu’il contient appartient à la personne dont il porte le nom ({ $folders }) ; le reste suit la règle au-dessus.
folder-rule-result = La règle de partage de { $folder } est enregistrée sur cet ordinateur. Les autres l’appliqueront après leur prochaine vérification du partage. Consultez la règle du dossier sur un autre ordinateur pour vérifier son arrivée.
problem-folder-rule-path = Choisissez un sous-dossier au nom portable, avec son orthographe d’origine. La racine du groupe et les fichiers propres à pigeon ne peuvent pas recevoir de règle.
problem-folder-rule-person = Aucun membre actuel unique ne correspond à « { $name } ». Choisissez son nom dans Personnes et machines, ou utilisez me pour vous-même.
problem-rules-raise = Cette règle de partage demande les règles { $rules } du groupe. Un ordinateur dont le pigeon ne les connaît pas attend les modifications du groupe jusqu’à sa mise à jour. Les autres ont signalé en dernier : { $computers }.
hint-rules-raise = Mettez d’abord ces ordinateurs à jour si possible, puis confirmez, ou relancez avec --yes.
confirm-rules-raise = Passer le groupe à ces règles et enregistrer cette règle de partage ? [o/N]
web-folder-rule = Règle de partage du dossier
web-folder-rule-help = Choisissez qui peut modifier ce dossier. Les autres peuvent suggérer des modifications. Les sous-dossiers héritent de cette règle sauf s’ils ont la leur.
web-folder-rule-admin = Un administrateur peut modifier cette règle de partage.
web-folder-rule-owner = Une seule personne peut le modifier
web-folder-rule-add-only = Tout le monde peut ajouter ; les personnes choisies décident des autres modifications
web-folder-rule-inherit = Hériter de la règle du dossier parent
web-folder-rule-per-person = Chaque dossier qu’il contient appartient à la personne dont il porte le nom
web-folder-rule-confirm-rules = Si cela demande des règles plus récentes, passer le groupe à ces règles : un ordinateur dont le pigeon ne les connaît pas attend les modifications jusqu’à sa mise à jour
web-folder-rule-stewards = Personnes qui décident des modifications
web-folder-rule-default-admins = Ne cochez personne pour laisser les administrateurs décider.

web-view-suggestion = Examiner le fichier suggéré
suggestion-deletes-file = Cette suggestion supprime { $path }. Le fichier partagé reste inchangé tant que la suggestion n’est pas acceptée.

space-title = Espace de sauvegarde
space-need =
    { $copies ->
        [one] Stockage nécessaire pour { $copies } copie de sauvegarde : fichiers actuels { $current } ; historique { $history } ; suggestions { $suggestions }.
       *[other] Stockage nécessaire pour { $copies } copies de sauvegarde : fichiers actuels { $current } ; historique { $history } ; suggestions { $suggestions }.
    }
space-offered = Capacité configurée pour ce contenu : { $bytes }. Chaque ordinateur de sauvegarde compte pour une copie au maximum, même sans état de stockage récent.
space-overflow = Copies supplémentaires : { $bytes }, selon les emplacements signalés.
space-overflow-unknown = Les copies supplémentaires n’ont pas été vérifiées.
space-limit-low = La limite de stockage de { $machines } est inférieure à une copie des fichiers du groupe : cette machine ne peut pas tout garder. Une autre machine de sauvegarde, ou une limite plus haute, compense.
space-no-spare = Il n’y a pas de machine de sauvegarde en réserve : si l’une d’elles cesse de fonctionner, vos fichiers ne sont plus en sécurité jusqu’à son remplacement. Ajouter une autre machine de sauvegarde évite cela.
space-disk = Stockage utilisé { $used } ; libre { $free } ; limite { $limit } ; réserve { $reserve }.
space-proposal = Une suggestion enregistrée { $when }, { $state } : { $bytes } de plus sur chaque machine de sauvegarde.
space-shared = Un contenu commun est compté une seule fois dans les totaux et peut apparaître dans plusieurs suggestions. Les suggestions refusées restent jusqu’à leur effacement.
space-open = en attente d’une décision
space-adopted = acceptée
space-rejected = refusée
space-withdrawn = retirée
space-snapshot = { $group }, vérifié { $when }.

# Explicit, reference-aware erasure (08 §8.6).
erase-checking = L’effacement attend la vérification de l’historique et la récupération locale.
erase-prepare-needed = Un administrateur doit préparer l’effacement. Chaque ordinateur actuel doit confirmer la mise à jour avant tout effacement.
erase-readiness-wait = L’effacement attend la fin de la préparation des ordinateurs et des machines de sauvegarde.
erase-ready = Tous les ordinateurs actuels ont confirmé la préparation. Examinez cette portée avant de demander l’effacement.
erase-waiting = En attente de : { $names }.
erase-old-decisions = Un effacement antérieur doit se terminer ou être annulé avant qu’une nouvelle sauvegarde participe aux effacements.
erase-preview-counts = Cette action retire { $files } fichiers actuels et demande l’effacement de { $versions } versions observées, couvrant { $bytes } octets distincts.
erase-warning = L’effacement peut supprimer définitivement ces versions. Chaque sauvegarde attend au moins { $days } jours après réception de la demande, et toutes les sauvegardes requises doivent donner leur accord. Restaurer une version avant la fin la conserve. Ceci n’est pas un effacement sécurisé des disques ni des copies hors de Pigeon.
erase-shared = { $versions } versions sélectionnées partagent des octets conservés ailleurs : { $paths }. Ces octets restent ; la taille affichée ne garantit pas l’espace qui sera libéré.
erase-needs-approval = Cette action envoie une suggestion à un administrateur. Aucun effacement ne commence avant son acceptation.
erase-preparing = Préparation demandée. Aucun effacement n’a été effectué ni programmé. Examinez et confirmez à nouveau lorsque tous les ordinateurs seront prêts.
erase-submitted = Effacement demandé. Reçu : { $id }. Consultez les demandes d’effacement pour connaître les sauvegardes attendues et l’achèvement.
erase-submitted-suggestion = La suggestion d’effacement { $id } attend un administrateur. Le délai commence après acceptation et réception par chaque sauvegarde.
erase-none = Aucune demande d’effacement acceptée.
erase-decision = { $id } : { $paths }. Fin déclarée au plus tôt : { $time } ; le délai propre à chaque destinataire s’applique aussi.
erase-completed = Chaque sauvegarde d’origine a validé cette portée. Les références hors de cette portée ou partagées conservent leurs octets.
erase-pending = En attente du délai de conservation et des validations de toutes les sauvegardes requises. Cette demande couvre les versions examinées lors de sa création. Les enregistrements et restaurations ultérieurs sont conservés ; restaurer n’annule pas cette demande.
erase-membership-changed = La liste des sauvegardes a changé avant la fin. Un administrateur doit annuler cette demande puis en examiner une nouvelle.
erase-cancel-warning = Annuler l’effacement en attente { $id } ? L’annulation ne peut pas reconstituer des octets déjà supprimés. Les fichiers actuels ne sont pas restaurés automatiquement.
erase-cancelled = Effacement { $id } annulé. Restaurez séparément les fichiers actuels que vous souhaitez retrouver.
confirm-erase = Demander cet effacement ? [o/N]
confirm-erase-prepare = Préparer l’effacement pour ce groupe sans demander d’effacement ? [o/N]
cmd-erase = Examiner l’effacement définitif des versions observées ou consulter son attente.
problem-erase-stale = Un nouvel historique ou des confirmations de préparation sont arrivés depuis votre examen. Cette tentative n’a rien modifié. Ouvrez un nouvel aperçu, examinez sa portée, puis confirmez à nouveau.
problem-erase-pending = Terminez l’opération locale interrompue avant de demander un autre effacement.
problem-erase-not-ready = L’effacement n’est pas prêt. Consultez pigeon erase et l’aperçu pour connaître les ordinateurs ou sauvegardes attendus.
problem-erase-checking = L’effacement attend la vérification de l’historique ou la récupération locale. Consultez Diagnostic pour plus de détails.
problem-erase-unknown = Cet identifiant ne désigne pas un effacement en attente unique. La commande pigeon erase affiche les demandes actuelles.

web-erase-title = Effacer les versions observées
web-erase-list = Demandes d’effacement
web-erase-confirm = Demander cet effacement
web-erase-suggest = Suggérer cet effacement à un administrateur
web-erase-prepare = Préparer l’effacement uniquement
web-erase-cancel = Annuler cet effacement en attente
web-erase-review-cancel = Examiner l’annulation de
web-erase-suggestion = Examiner l’effacement de cette suggestion

# Integrity pass evidence and local progress (08 §8.8).
scrub-no-pass = Aucun contrôle d’intégrité complet n’a été signalé.
scrub-last-pass = Dernier contrôle d’intégrité complet : { $time }. Anomalies trouvées pendant ce contrôle : { $failed }.
scrub-progress = Contrôle d’intégrité local en cours : { $remaining } contenus restants. Anomalies trouvées : { $failed }.
scrub-unreadable = La progression du contrôle d’intégrité est illisible. Les données existantes sont conservées ; consultez Diagnostic.
scrub-machine = Contrôles d’intégrité sur { $machine } :
problem-scrub-ledger = L’état du contrôle d’intégrité sur { $machine } est illisible. Conservez-le et rassemblez un rapport de diagnostic avant de réparer l’état local.
scrub-report-time = Signalé { $when }. Un nouveau contact ne met pas cette observation à jour.
erase-needs-backup = Ajoutez une machine de sauvegarde avant de préparer l’effacement.
erase-preparation-split = Les ordinateurs ont accepté des préparations différentes ; l’effacement ne peut donc pas commencer. Un administrateur peut créer un groupe distinct avec pigeon refound, y inviter à nouveau les ordinateurs, puis y préparer l’effacement. Les demandes de l’ancien groupe restent dans cet ancien groupe.
problem-erase-needs-backup = Ajoutez une machine de sauvegarde avant de préparer l’effacement.

web-health-suggestions-unavailable = Suggestions en attente d’une décision : pas encore vérifiées.
web-health-separator = {" : "}
web-not-found = Il n’y a pas de page ici.
web-noscript = Cette page ne se met à jour d’elle-même qu’avec JavaScript. Rechargez-la pour voir l’état le plus récent.
web-skip = Aller au contenu
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
web-sections = Sections
web-status = État :
web-breadcrumb = Emplacement
web-size-b = { $size } o
web-size-kb = { $size } ko
web-size-mb = { $size } Mo
web-size-gb = { $size } Go
web-size-tb = { $size } To
web-decimal-separator = {","}
web-folder-all-safe = Tout est en sécurité
web-folder-not-safe =
    { $count ->
        [one] 1 pas encore en sécurité
       *[other] { $count } pas encore en sécurité
    }
web-folder-files =
    { $files ->
        [one] 1 fichier
       *[other] { $files } fichiers
    }
web-files-lead = Votre dossier partagé sur cet ordinateur : { $folder }
web-files-empty-help = Les fichiers que vous mettez dans { $folder } apparaissent ici dès que pigeon les a vus.
web-folder-freed-note = Ce dossier n'est pas sur cet ordinateur : ses fichiers sont gardés en sécurité sur les machines de sauvegarde. Gardez-le sur cet ordinateur pour le récupérer, complet.
web-filter = Filtrer par nom
web-filter-placeholder = Filtrer par nom…
web-filter-none = Aucun nom ici ne contient ce que vous avez tapé.
web-files-caption = Dossiers et fichiers, avec leur état, leur taille et leur dernière modification
web-col-kind = Type
web-col-name = Nom
web-col-state = État
web-col-size = Taille
web-col-modified = Modifié
web-col-deleted = Supprimé
web-col-actions = Actions
web-not-here = Pas sur cet ordinateur
web-kept-here = Gardé sur cet ordinateur
web-details = Détails
web-details-of = Détails de { $name }
web-details-close = Fermer les détails de { $name }
web-versions-of = Versions de { $name }
web-version-number = Version { $number }
web-saved = Enregistrée
web-current-version = version actuelle
web-detail-files = Contient
web-detail-here = Sur cet ordinateur
web-detail-rule = Qui peut le modifier
web-open-folder = Ouvrir
web-all-versions = Toutes les versions
web-readme-too-long = { $name } est trop long pour être affiché ici ({ $size }).
web-readme-not-here = { $name } n'est pas sur cet ordinateur, il n'est donc pas affiché ici. Gardez ce dossier sur cet ordinateur pour le lire ici.
web-readme-open = Voir ses versions et en ouvrir une copie
web-task-done = Fait :
web-task-open = À faire :
web-deleted-lead = Les fichiers supprimés restent en sécurité : restaurez-en un pour retrouver sa dernière version, avec son historique.
web-suggestions-empty-help = Quand quelqu'un modifie un fichier dont vous décidez, sa modification vous attend ici.
web-people = Personnes
web-machines = Ordinateurs et machines de sauvegarde
web-copied = Copié
web-settings-computer = Cet ordinateur
web-settings-sharing = Partage
web-leave-help = Cet ordinateur arrête de partager ; ses fichiers restent dans le dossier.
web-rule-owned = Seulement { $owner } ; les modifications des autres deviennent des suggestions
web-rule-add-only = Tout le monde peut ajouter ; les autres modifications sont décidées par { $stewards }
web-rule-per-person = Chaque dossier qu'il contient appartient à la personne dont il porte le nom
web-shortcuts = Aller à
web-here-others = Dossier de { $owner }, pas sur cet ordinateur
web-here-chosen = Votre choix
web-forget = Revenir au choix par défaut
web-folder-others-note = C'est le dossier de { $owner } : il n'est pas sur cet ordinateur, sauf si vous le gardez. Ses fichiers restent en sécurité sur les machines de sauvegarde.
web-sync-title = Dossiers sur cet ordinateur
web-sync-lead = Cochez les dossiers à garder sur cet ordinateur. Un dossier coché ou décoché entraîne les dossiers qu'il contient ; vous pouvez encore choisir pour chacun d'eux.
web-sync-help = Les fichiers d'un dossier qui n'est pas sur cet ordinateur restent en sécurité sur les machines de sauvegarde, et pigeon les montre toujours. Libérer un dossier attend que chacun de ses fichiers y soit en sécurité ; le garder à nouveau le télécharge, complet.
web-sync-partly = certains de ses dossiers ne sont pas sur cet ordinateur
web-sync-inside =
    { $count ->
        [one] 1 dossier dans { $name }
       *[other] { $count } dossiers dans { $name }
    }
web-sync-apply = Appliquer
web-sync-unchanged = Rien à changer : chaque dossier est déjà comme coché.

headline-erased-restoration-missing =
    { $files ->
        [one] La restauration d’un fichier n’a pas pu se terminer : son contenu manque. Essayez une autre version enregistrée ou récupérez une copie à part.
       *[other] La restauration de { $files } fichiers n’a pas pu se terminer : leur contenu manque. Essayez d’autres versions enregistrées ou récupérez des copies à part.
    }
why-erased-restoration-missing = L’effacement précédent couvrait ce contenu. Votre demande de restauration reste enregistrée, mais aucune copie n’est connue pour le moment. Si vous trouvez une copie, réessayez la restauration.

web-settings-resources = Utilisation de cet ordinateur
web-settings-resources-help = Discret laisse le partage en arrière-plan. Vos réglages détaillés remplacent le préréglage. Les débits sont en octets par seconde ; séparez les plages HH:MM-HH:MM selon l’heure locale de cet ordinateur par des virgules. Les plages à pleine vitesse désactivent le ralentissement automatique ; vos limites d’envoi et de réception restent applicables. Les attentes liées à la batterie et au réseau limité restent applicables pendant ces plages.
web-settings-resources-restart-help = Avec le service installé de pigeon, passer à Rapide ou en sortir redémarre brièvement pigeon. Si la page se déconnecte, attendez un instant puis rechargez-la. Les changements entre Discret et Équilibré ne redémarrent pas pigeon.
setting-resources-preset = Préréglage
setting-resources-upload = Envoi maximal (auto ou 2 MB/s)
setting-resources-download = Réception maximale (auto ou 2 MB/s)
setting-resources-on-battery = Quand la batterie est faible
setting-resources-battery-percent = Seuil de batterie faible (%)
setting-resources-on-metered = Sur un réseau à consommation limitée
setting-resources-full-speed = Plages à pleine vitesse (vide : aucune)
setting-resources-scrub-window = Plage de vérification
setting-gentle = Discret
setting-balanced = Équilibré
setting-fast = Rapide
setting-pause = Attendre
setting-continue = Continuer
headline-resource-battery = En attente de courant : cet ordinateur utilise sa batterie.
headline-resource-metered = En attente d’un réseau sans limite de consommation.
headline-resource-disk = En attente pendant qu’un autre programme utilise le disque.
headline-resource-saving = Les transferts attendent pendant l’enregistrement des dernières modifications. Ils reprendront automatiquement.

setting-none = Aucune

web-settings-resources-power-help = Avec Attendre, les transferts de fichiers attendent sous le seuil de batterie ou en mode économie d’énergie, puis reprennent automatiquement quand les conditions s’améliorent. Les attentes liées au réseau limité prennent aussi fin automatiquement. Reprendre le partage met fin à une pause que vous avez choisie.
web-resource-value-invalid = « { $value } » ne convient pas pour { $key }. Rien n’a changé. Revenez en arrière et utilisez un choix proposé ou le format indiqué pour ce réglage.

status-scan-only = Pigeon ne peut pas surveiller ce dossier pour le moment. Il vérifie régulièrement si des fichiers ont changé et réessaie automatiquement.
