# The Blob spiegato semplice — 20 concetti da conoscere

Questa pagina raccoglie, in linguaggio volutamente semplice, i **20 argomenti architetturali che restano da approfondire** dopo i concetti fondamentali già discussi: Personal World, Compute Fabric, Workspace, Workspace Recipe, EnvironmentSpec/Nix, Object, chunk, Content-Addressed Store, Manifest, Workspace Head, Mutation Journal, Durability, replica, Replication Controller, Workspace Handoff, pre-copy, prewarming e lazy materialization.

Non sostituisce [`ARCHITECTURE.md`](ARCHITECTURE.md) o [`CONCEPTS.md`](CONCEPTS.md). È la loro versione didattica: serve a capire *che cosa significa ogni parola* prima di entrare nei dettagli tecnici.

## Indice

1. [Write Lease e conflitti](#1-write-lease-e-conflitti)
2. [Surface](#2-surface)
3. [Task, Capability e Capability Requirement](#3-task-capability-e-capability-requirement)
4. [Capability Capsule](#4-capability-capsule)
5. [Resolver e Constraint Solver](#5-resolver-e-constraint-solver)
6. [Late Binding e Re-resolution](#6-late-binding-e-re-resolution)
7. [Alfred / Situation Engine](#7-alfred--situation-engine)
8. [Knowledge Object](#8-knowledge-object)
9. [Temporal Graph e Causal Graph](#9-temporal-graph-e-causal-graph)
10. [Identity, Trust, Authorization e Capability Grants](#10-identity-trust-authorization-e-capability-grants)
11. [Constitutional Core](#11-constitutional-core)
12. [Adaptive System](#12-adaptive-system)
13. [Offline e partizioni del Fabric](#13-offline-e-partizioni-del-fabric)
14. [Process/VM migration vs semantic migration](#14-processvm-migration-vs-semantic-migration)
15. [Garbage collection ed eviction](#15-garbage-collection-ed-eviction)
16. [Recovery dopo un crash](#16-recovery-dopo-un-crash)
17. [Security e cifratura del CAS](#17-security-e-cifratura-del-cas)
18. [Multi-user e condivisione](#18-multi-user-e-condivisione)
19. [Il ruolo dell'AI](#19-il-ruolo-dellai)
20. [Architettura completa end-to-end](#20-architettura-completa-end-to-end)

---

## 1. Write Lease e conflitti

Un **Write Lease** è il permesso temporaneo di essere il nodo che può modificare la versione principale di un Workspace.

Immagina una lavagna e un solo pennarello speciale. Desktop e notebook possono entrambi vedere la lavagna, ma soltanto chi possiede il pennarello può scrivere sulla versione principale.

```text
Workspace Development

Desktop  -> writer
Notebook -> replica pronta
```

Quando spostiamo il Workspace:

```text
Desktop -- passa il Write Lease --> Notebook
```

Il notebook diventa writer e il desktop può restare una replica. Questo evita che due macchine modifichino per sbaglio lo stesso stato contemporaneamente.

Se vogliamo davvero lavorare su due macchine nello stesso momento, non dobbiamo fingere che il conflitto non esista: possiamo creare due **branch**, cioè due rami dello stesso Workspace, e unirli in seguito con un merge controllato.

**Idea da ricordare:** una replica può avere tutti i dati senza avere automaticamente il diritto di modificare la linea principale.

---

## 2. Surface

Il **Workspace** è il lavoro logico. La **Surface** è il modo in cui quel lavoro viene mostrato su un certo dispositivo.

Lo stesso Workspace `Development` potrebbe apparire così:

```text
Desktop -> editor + terminale + test + documentazione
Telefono -> stato task + notifiche + piccole approvazioni
Watch -> "test completati" + pulsante OK
```

Non abbiamo tre Workspace. Abbiamo **un Workspace e tre Surface**.

È come lo stesso libro: sul monitor lo vediamo a due pagine, sul telefono a una pagina, sullo smartwatch magari soltanto come notifica. Il contenuto logico rimane lo stesso.

Lo stato importante della Surface deve essere descritto in modo strutturato, per esempio “documento attivo = ARCHITECTURE.md” invece di “pixel alle coordinate 812,240”. In questo modo anche gli agenti possono capire l'interfaccia senza dover indovinare guardando screenshot.

**Idea da ricordare:** `Workspace != Surface`.

---

## 3. Task, Capability e Capability Requirement

Un **Task** è una cosa concreta che vogliamo ottenere.

Esempio:

```text
Task:
"Traduci questo documento in inglese"
```

Una **Capability** è una capacità astratta, cioè “una cosa che il sistema sa fare”.

```text
Capability:
document.translate
```

Il **Capability Requirement** descrive con precisione che tipo di capacità serve e con quali regole.

```text
serve: document.translate
input: documento
output: documento tradotto
privacy: solo locale
qualità: alta
```

Il Task quindi **non dice** “avvia il programma X sul server Y”. Dice che risultato vuole e che capacità gli serve.

Per Task più complessi The Blob usa un **Requirement Graph**: una mappa con più ruoli e relazioni da soddisfare insieme.

**Idea da ricordare:** il Task descrive il lavoro; la Capability descrive ciò che serve saper fare; il Requirement descrive i vincoli che quella capacità deve rispettare.

---

## 4. Capability Capsule

La **Capability** è astratta. La **Capability Capsule** è l'implementazione concreta che la realizza.

Esempio:

```text
Capability:
model.inference
```

Potrebbe essere realizzata da:

```text
- un programma nativo
- un componente WASM
- un container OCI
- una microVM
- un modello AI locale
- un servizio remoto
- hardware specializzato
```

È come dire “mi serve un mezzo per andare a scuola”. La Capability è `transport`; la Capsule concreta potrebbe essere autobus, auto o bicicletta.

The Blob può quindi sostituire l'implementazione senza cambiare il significato del Task.

Una Capsule è normalmente **usa-e-getta rispetto allo stato dell'utente**: possiamo distruggerla e ricrearla. Il Workspace, i documenti e il Task devono invece sopravvivere.

**Idea da ricordare:** `Capability != implementation` e `Capsule lifetime != user-state lifetime`.

---

## 5. Resolver e Constraint Solver

Il **Resolver** è il componente che cerca una soluzione concreta per un Requirement.

Esempio:

```text
Task: esegui inferenza AI

Desktop: GPU potente, occupato
Notebook: GPU piccola, vicino all'utente
Server: GPU potente, libero
Cloud: potente, ma vietato dalla privacy
```

Il Resolver raccoglie i candidati. Il **Constraint Solver** verifica combinazioni e vincoli come:

```text
privacy
fiducia
CPU/GPU
RAM
batteria
latenza
costo
rete
qualità
```

The Blob non vuole però affidare l'autorità al solver. Il solver produce una proposta di **Binding Plan**, cioè “questa è la combinazione che sceglierei”; un verificatore Rust più semplice e indipendente controlla poi che quella proposta rispetti davvero le regole.

È simile a un organizzatore che prepara un piano e a un controllore separato che verifica che il piano sia valido prima di autorizzarlo.

**Idea da ricordare:** il solver può trovare una buona soluzione; non può rendere valida una soluzione vietata.

---

## 6. Late Binding e Re-resolution

**Binding** significa collegare una richiesta astratta a una soluzione concreta.

```text
model.inference
        ↓
server + modello X
```

**Late Binding** significa rimandare questa scelta il più possibile, invece di fissarla troppo presto.

Perché? Perché la situazione cambia.

Alle 10:00 il server può essere il migliore. Alle 10:05 potrebbe essere spento e il desktop potrebbe essere diventato la scelta migliore.

La **Re-resolution** significa rifare la scelta quando cambiano le condizioni, ma soltanto in punti sicuri.

Non vogliamo spostare arbitrariamente una cosa nel mezzo di un'operazione irreversibile. Per questo il risultato della risoluzione può essere protetto da un **Binding Lease**, un impegno temporaneo che dice anche quando è consentito riconsiderare la scelta.

**Idea da ricordare:** The Blob sceglie *tardi* e può scegliere di nuovo, ma non in modo caotico.

---

## 7. Alfred / Situation Engine

**Alfred** è il sistema nervoso di The Blob.

Non esegue semplicemente comandi. Osserva eventi nel tempo e cerca di capire quando insieme descrivono una situazione importante.

Esempio:

```text
notebook scollegato dalla corrente
+ Wi-Fi sparito
+ telefono in movimento
+ task lungo ancora attivo
        ↓
Situation:
"L'utente sta andando via mentre un lavoro è ancora in corso"
```

A quel punto Alfred può proporre:

```text
- sposta il Task sul server
- prepara una Surface sul telefono
- chiedi conferma all'utente
```

La pipeline deve restare separata:

```text
eventi
↓
correlazione deterministica
↓
interpretazione semantica anche con AI
↓
Situation strutturata
↓
policy e autorizzazione deterministiche
↓
eventuale azione
```

**Idea da ricordare:** Alfred può capire che “sta succedendo qualcosa”, ma capire non significa automaticamente avere il permesso di agire.

---

## 8. Knowledge Object

Un **Knowledge Object** è una cosa persistente del Personal World con una propria identità, non soltanto un file in una cartella.

Esempio:

```text
Knowledge Object:
"Architettura di The Blob"
```

Può avere:

```text
contenuto
metadati
relazioni
provenienza
storia
```

E può avere diverse **Representation**:

```text
Markdown
PDF
HTML
audio
thumbnail
```

Il PDF e il Markdown non devono necessariamente essere due “documenti concettualmente diversi”: possono essere due rappresentazioni dello stesso Knowledge Object.

Una **Projection** è invece soltanto una parte autorizzata dell'Object, per esempio la sezione “Architecture”, mentre una **View** è una query salvata che raccoglie Object secondo un criterio.

**Idea da ricordare:** il file è una forma fisica; il Knowledge Object è la cosa persistente che vogliamo conoscere, collegare e versionare.

---

## 9. Temporal Graph e Causal Graph

The Blob deve ricordare due cose differenti:

1. **com'era lo stato prima?**
2. **perché è cambiato?**

Il lato **Temporal** conserva versioni nel tempo:

```text
W40 -> W41 -> W42
```

Il lato **Causal** spiega:

```text
cosa è cambiato
chi o quale agente l'ha proposto
quale evento lo ha causato
perché è stato autorizzato
quale risultato ci aspettavamo
quale risultato abbiamo ottenuto
come tornare indietro
```

Esempio: sapere che Python è passato dalla versione A alla B è storia temporale. Sapere che è stato aggiornato perché serviva una libreria, che il test è passato e che Antonio ha autorizzato il cambio è storia causale.

**Idea da ricordare:** `state history != causal explanation`.

---

## 10. Identity, Trust, Authorization e Capability Grants

Questi termini sembrano simili ma rispondono a domande diverse.

**Identity**: chi sei?

```text
questo utente = Antonio
questo nodo = notebook personale
```

**Trust**: quanto ci fidiamo di quel nodo, servizio o componente?

**Authorization**: ha il permesso di fare questa specifica cosa?

Un **Capability Grant** è un permesso molto stretto e temporaneo.

Invece di dare a un agente “accesso completo alla posta”, possiamo concedere:

```text
mail.send
recipient = X
max_messages = 1
expires = 10 minuti
```

È come dare a qualcuno la chiave di un singolo armadietto per dieci minuti invece della chiave dell'intero edificio.

**Idea da ricordare:** conoscere l'identità non significa fidarsi; fidarsi non significa essere autorizzati; l'autorizzazione deve essere il più stretta possibile.

---

## 11. Constitutional Core

Il **Constitutional Core** è la parte minima di The Blob che deve rimanere affidabile anche se tutto il resto cambia.

Protegge cose come:

```text
identità
policy fondamentali
autorizzazione
verifica
trusted boot/recovery
rollback
```

Immagina una casa intelligente che può cambiare mobili, pareti mobili e dispositivi, ma non può decidere da sola di eliminare la porta antincendio o consegnare le chiavi a uno sconosciuto.

L'Adaptive System e l'AI possono proporre cambiamenti, ma non devono poter disattivare normalmente il sistema che verifica e autorizza quei cambiamenti.

**Idea da ricordare:** è il custode delle regole fondamentali, non il componente che fa tutto.

---

## 12. Adaptive System

L'**Adaptive System** è la parte modificabile del sistema operativo e del runtime.

Può comprendere:

```text
configurazione
servizi
runtime
parametri kernel
driver
moduli
profili prestazionali
```

The Blob può osservare un problema e proporre un cambiamento:

```text
problema
↓
proposta
↓
branch candidato
↓
build isolata
↓
test / simulazione / benchmark
↓
autorizzazione
↓
attivazione controllata
↓
misura del risultato
↓
commit oppure rollback
```

Quindi “sistema adattivo” non significa “l'AI modifica liberamente il kernel”. Significa che i cambiamenti possono essere proposti e sperimentati con una procedura controllata e reversibile.

**Idea da ricordare:** adattarsi sì; auto-modificarsi senza verifica no.

---

## 13. Offline e partizioni del Fabric

Una **partizione** significa che due parti del Fabric temporaneamente non riescono a comunicare.

Esempio:

```text
Desktop + NAS       Notebook
      |                 |
      X---- rete ----X
```

Il notebook deve poter continuare a lavorare con ciò che possiede localmente.

Quando la connessione torna, The Blob deve capire:

```text
quali Object sono cambiati?
quali mutation mancano?
ci sono conflitti?
chi possiede il Write Lease valido?
quali repliche devono convergere?
```

La rete non deve essere considerata “rotta” solo perché è temporaneamente disconnessa. L'offline è una modalità normale del Personal World.

Qui possiamo imparare da Coda, sistemi offline-first, Iberna e dalle idee store-and-forward studiate anche in PollicinoNet.

**Idea da ricordare:** il Fabric deve degradare bene quando la rete sparisce e riconciliarsi quando ritorna.

---

## 14. Process/VM migration vs semantic migration

Ci sono due modi molto diversi di “continuare altrove”.

### Migrazione pesante

Congeliamo quasi tutto lo stato di un processo o di una VM, compresa la RAM, e lo riprendiamo altrove.

Iberna lo fa già in parte quando usa `vagrant suspend`: è come congelare una stanza intera e scongelarla dopo.

### Migrazione semantica

Salviamo invece soltanto ciò che *significa* continuare il lavoro:

```text
file aperti
Task
contesto
Workspace state
Surface state
Object necessari
```

Poi ricreiamo runtime e Capsule sul nuovo nodo.

La migrazione semantica è normalmente più leggera, più portabile e può funzionare anche fra architetture diverse. La migrazione RAM/VM rimane utile per casi speciali come simulatori, REPL particolari o calcoli difficili da ricostruire.

**Idea da ricordare:** normalmente vogliamo spostare il lavoro, non necessariamente ogni byte della RAM del programma.

---

## 15. Garbage collection ed eviction

Con il tempo il CAS può accumulare moltissimi chunk, versioni, cache e snapshot.

**Eviction** significa togliere una copia locale non più utile.

```text
modello X sul notebook -> rimosso
modello X sul NAS -> ancora presente
```

L'Object non è stato cancellato dal Personal World: abbiamo soltanto liberato una cache locale.

La **Garbage Collection** (GC) elimina invece dati che non sono più raggiungibili o necessari secondo le regole di conservazione.

Prima di eliminare qualcosa davvero, The Blob deve verificare riferimenti, retention, durability, versioni e recovery requirements.

È come riordinare un magazzino: buttare una scatola duplicata è diverso dal buttare l'ultima copia di un documento importante.

**Idea da ricordare:** eviction libera una replica; GC decide quando un dato non serve più davvero.

---

## 16. Recovery dopo un crash

Un **crash** è un'interruzione improvvisa: programma morto, corrente saltata, sistema riavviato, nodo sparito.

The Blob deve assumere che possa succedere anche nel momento peggiore:

```text
Desktop sta trasferendo il Write Lease
Notebook sta ricevendo le ultime mutation
          ↓
         CRASH
```

Al riavvio non deve “indovinare” chi era writer.

Deve ricostruire la verità da evidenze durabili:

```text
ultimo Workspace Head valido
Mutation Journal persistente
stato del lease
repliche confermate
operazioni in-flight
eventi causali
```

Se il risultato di un'operazione non è conoscibile con certezza, il sistema deve segnalarlo come **unknown/reconciliation required**, non inventare un successo.

**Idea da ricordare:** il recovery corretto vale più di un'interfaccia che finge che tutto sia andato bene.

---

## 17. Security e cifratura del CAS

Il fatto che un chunk abbia un hash non significa che sia segreto.

Se conserviamo Object su:

```text
notebook
NAS
server
cloud
```

dobbiamo decidere chi può leggerli.

La cifratura serve a trasformare i dati in qualcosa di inutilizzabile senza la chiave appropriata.

Il problema è più sottile di “cifra tutto”: deduplicazione, hash, condivisione, key rotation, cancellazione e multi-user interagiscono fra loro.

Il modello deve separare almeno:

```text
identità dell'Object
integrità
cifratura
chiavi
autorizzazione
replica
```

Un nodo può essere autorizzato a **conservare** un ciphertext senza essere autorizzato a **leggere** il contenuto.

**Idea da ricordare:** storage e diritto di lettura sono due cose diverse.

---

## 18. Multi-user e condivisione

Il Personal World nasce centrato su un utente, ma alcuni Object, Task o Workspace possono essere condivisi.

Condividere non dovrebbe significare:

```text
"ti do accesso al mio intero Personal World"
```

ma piuttosto:

```text
Object X -> condiviso con Maria
Projection Y -> condivisa con il collega
Workspace Z -> collaborazione con il gruppo
```

Possiamo concedere permessi diversi:

```text
read
comment
edit
execute capability
share further: no
expires: venerdì
```

In futuro due utenti potrebbero anche collaborare su branch distinti e riconciliare il lavoro senza fondere le loro identità o i loro Personal World.

**Idea da ricordare:** condividiamo la minima porzione necessaria, non l'intero mondo personale.

---

## 19. Il ruolo dell'AI

L'AI è molto importante in The Blob, ma non è il “root” del sistema.

Può:

```text
capire intenzioni
interpretare Situations
proporre piani
spiegare problemi
suggerire Workspace Recipe
proporre modifiche al SystemSpec
scegliere tra alternative già valide
```

Non può da sola:

```text
inventare autorizzazioni
ignorare policy
rendere valido un Binding Plan proibito
disattivare il Constitutional Core
eseguire automaticamente qualunque comando privilegiato
```

La regola centrale dell'architettura è:

```text
AI interpreta, ragiona, propone e sintetizza.
Sistemi deterministici verificano, autorizzano e materializzano.
```

Questo permette di avere un sistema molto intelligente senza fare dell'LLM la parte di cui dobbiamo fidarci ciecamente.

**Idea da ricordare:** l'AI è il consigliere intelligente; non è il giudice che assegna a se stesso i permessi.

---

## 20. Architettura completa end-to-end

Mettiamo tutto insieme con un esempio.

Antonio apre il Workspace `Development` sul desktop.

```text
Personal World
      ↓
Workspace Development
      ↓
Workspace Recipe
      ↓
EnvironmentSpec -> Nix prepara l'ambiente
      ↓
Workspace Head + Manifest
      ↓
CAS materializza gli Object necessari
      ↓
Surface desktop
```

Antonio modifica il codice:

```text
modifica
↓
RAM/page cache
↓
Mutation Journal
↓
local durability
↓
replica su NAS/server
```

Vuole eseguire un test AI:

```text
Task
↓
Requirement Graph
↓
Capability richiesta
↓
Resolver + Constraint Solver
↓
Binding Plan
↓
verifica indipendente
↓
Binding Lease
↓
Capsule sul nodo migliore del Compute Fabric
↓
risultato
```

Alfred osserva che Antonio sta per uscire e che il Workspace potrebbe servire sul notebook. Il Replication Controller aveva già fatto prewarming e pre-copy.

Antonio trascina il Workspace sul notebook:

```text
prepare target
↓
manda solo Object/mutation mancanti
↓
Durability Barrier
↓
trasferisci Write Lease
↓
materializza Surface notebook
↓
continua a lavorare
```

I dati freddi mancanti arrivano in lazy materialization mentre le repliche continuano a convergere in background.

Se qualcosa va storto, il Constitutional Core e il recovery usano stato durabile e causal history per tornare a una situazione verificabile.

Questa è la visione complessiva:

```text
              PERSONA
                ↓
          PERSONAL WORLD
                ↓
             WORKSPACE
          ↙             ↘
   Knowledge/Object     Task/Goal
          ↓                ↓
       CAS/State      Requirements
          ↓                ↓
          └────── FABRIC ──┘
                    ↓
        capacità + nodi concreti
                    ↓
             risultato verificato
                    ↓
       stato persistente + storia
```

**Idea da ricordare:** The Blob vuole rendere persistenti il lavoro, il significato e l'intento dell'utente; le singole macchine e le singole implementazioni diventano risorse sostituibili del Fabric.

---

## Mini-glossario

| Termine | Spiegazione corta |
|---|---|
| Write Lease | Permesso temporaneo di scrivere sulla linea principale di un Workspace |
| Surface | Come un Workspace viene mostrato su un dispositivo |
| Task | Lavoro concreto da completare |
| Capability | Cosa il sistema sa fare in astratto |
| Capability Requirement | Capability richiesta più vincoli |
| Capability Capsule | Implementazione concreta di una Capability |
| Resolver | Cerca una soluzione concreta per un Requirement |
| Constraint Solver | Verifica/compara combinazioni sotto vincoli |
| Binding Plan | Soluzione concreta proposta per un Requirement Graph |
| Binding Lease | Impegno temporaneo su un Binding Plan e i suoi limiti di re-resolution |
| Late Binding | Scegliere l'implementazione/nodo il più tardi possibile |
| Re-resolution | Rifare una scelta quando cambiano le condizioni |
| Alfred | Sistema nervoso event-driven che riconosce Situations |
| Knowledge Object | Oggetto persistente con identità, storia, semantica e relazioni |
| Projection | Parte/visione semanticamente limitata di un Object |
| Representation | Forma materializzata: PDF, HTML, audio ecc. |
| Temporal Graph | Come cambia lo stato nel tempo |
| Causal Graph | Perché è cambiato e con quale conseguenza |
| Identity | Chi è un utente/nodo/componente |
| Trust | Quanto è considerato affidabile |
| Authorization | Che cosa è permesso fare |
| Capability Grant | Permesso stretto, limitato e possibilmente temporaneo |
| Constitutional Core | Base fidata che protegge regole, recovery e autorizzazione |
| Adaptive System | Parte modificabile e sperimentabile del sistema |
| Partition | Due parti del Fabric temporaneamente disconnesse |
| Semantic migration | Ricreare altrove il significato/stato necessario per continuare |
| Eviction | Togliere una copia locale non necessaria |
| Garbage Collection | Eliminare dati realmente non più necessari/raggiungibili |
| Recovery | Ricostruire uno stato affidabile dopo un'interruzione |

## Regola guida

Se una spiegazione futura di The Blob contraddice questa regola, deve essere riesaminata:

> **L'AI interpreta e propone. I meccanismi deterministici verificano e autorizzano. Il Personal World e lo stato dell'utente sopravvivono alle singole macchine, Capsule e runtime.**
