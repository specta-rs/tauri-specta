open DOMAPI // element, htmlInputElement types
open EventAPI // Click variant for addEventListener
open Bindings // Our generated bindings containing Commands and Events

let document = Global.document
external asInput: element => htmlInputElement = "%identity"

let greet = async () => {
  let input = document->Document.querySelector("#greet-input")->asInput
  let msg = document->Document.querySelector("#greet-msg")

  msg.textContent = {await Commands.helloWorld(~myName=input.value)}->Null.make
  setTimeout(() => Commands.goodbyeWorld()->Promise.thenResolve(Console.log)->ignore, 1000)->ignore
}

document
->Document.querySelector("#greet-button")
->Element.addEventListener(~type_=Click, ~callback=_ => greet()->ignore)

document
->Document.querySelector("#send-event-button")
->Element.addEventListener(~type_=Click, ~callback=_ => {
  Events.emptyEvent["emit"]()->ignore
})

let _ = Events.emptyEvent["listen"](_ => Console.log("Got event from frontend!!"))
let _ = Events.myDemoEvent["listen"](e => Console.log2("Got demo event:", e["payload"]))
