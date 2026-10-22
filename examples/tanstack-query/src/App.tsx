import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import reactLogo from "./assets/react.svg";
import { events, mutations, queries, queryKeys, type User } from "./bindings";
import "./App.css";

function App() {
  const [name, setName] = useState("");
  const [lastCreated, setLastCreated] = useState<User | null>(null);
  const queryClient = useQueryClient();

  // `queries.listUsers()` is typed from the Rust command and returns `queryOptions`, so it
  // is passed straight to `useQuery`.
  const users = useQuery(queries.listUsers());

  const createUser = useMutation({
    ...mutations.createUser(),
    onSuccess: () =>
      // `queryKeys` builds the same key `queries.listUsers()` reads, so invalidating the
      // prefix is enough to refetch the list.
      queryClient.invalidateQueries({ queryKey: queryKeys.listUsers() }),
  });

  // Events are typed exactly like the commands are.
  useEffect(() => {
    const unlisten = events.userCreated.listen(({ payload }) =>
      setLastCreated(payload.user),
    );

    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  return (
    <main className="container">
      <h1>Welcome to Tauri + React Query</h1>

      <div className="row">
        <a href="https://vite.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(event) => {
          event.preventDefault();
          createUser.mutate({ name });
          setName("");
        }}
      >
        <input
          id="name-input"
          value={name}
          onChange={(event) => setName(event.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit" disabled={createUser.isPending || !name.trim()}>
          Create user
        </button>
      </form>

      {lastCreated && (
        <p>
          Rust announced user #{lastCreated.id} over an event.
        </p>
      )}

      {users.isPending && <p>Loading users...</p>}
      {users.isError && <p>Could not load the users.</p>}
      {users.data && (
        <ul>
          {users.data.map((user) => (
            <li key={user.id}>
              #{user.id}: {user.name}
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

export default App;
