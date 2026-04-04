import reactLogo from "./assets/react.svg";
import "./App.css";
import { queries, mutations, queryKeys } from "./bindings";
import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

function UsersPanel({
  selectedUser,
  onSelectUser,
}: {
  selectedUser: number | null;
  onSelectUser: (id: number | null) => void;
}) {
  const queryClient = useQueryClient();
  const listUsersQuery = useQuery(queries.listUsers());

  const [newUserName, setNewUserName] = useState("");
  const [newUserEmail, setNewUserEmail] = useState("");

  const createUserMutation = useMutation({
    ...mutations.createUser(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.listUsers() });
      setNewUserName("");
      setNewUserEmail("");
    },
  });

  const deleteUserMutation = useMutation({
    ...mutations.deleteUser(),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.listUsers() });
      if (selectedUser === variables.id) {
        onSelectUser(null);
      }
    },
  });

  return (
    <div style={{ flex: 1 }}>
      <h2>Users</h2>

      <form
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem",
        }}
        onSubmit={(e) => {
          e.preventDefault();
          if (newUserName && newUserEmail) {
            createUserMutation.mutate({
              name: newUserName,
              email: newUserEmail,
            });
          }
        }}
      >
        <input
          placeholder="Name"
          value={newUserName}
          onChange={(e) => setNewUserName(e.target.value)}
        />
        <input
          placeholder="Email"
          value={newUserEmail}
          onChange={(e) => setNewUserEmail(e.target.value)}
        />
        <button type="submit" disabled={createUserMutation.isPending}>
          Add
        </button>
      </form>

      {listUsersQuery.isLoading && <p>Loading users...</p>}
      {listUsersQuery.isError && <p>Error loading users</p>}

      <ul style={{ listStyle: "none", padding: 0 }}>
        {listUsersQuery.data?.map((user) => (
          <li
            key={user.id}
            style={{
              padding: "0.5rem",
              marginBottom: "0.25rem",
              cursor: "pointer",
              borderRadius: "4px",
              background:
                selectedUser === user.id
                  ? "rgba(100, 108, 255, 0.2)"
                  : "transparent",
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
            }}
            onClick={() => onSelectUser(user.id)}
          >
            <span>
              <strong>{user.name}</strong> ({user.email})
            </span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                deleteUserMutation.mutate({ id: user.id });
              }}
              disabled={deleteUserMutation.isPending}
            >
              ×
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function TodosPanel({ userId }: { userId: number | null }) {
  const queryClient = useQueryClient();

  const getUserQuery = useQuery({
    ...queries.getUser(userId!),
    enabled: userId !== null,
  });

  const [filterTitle, setFilterTitle] = useState("");

  const todosQuery = useQuery({
    ...queries.listTodos(userId!, filterTitle || null),
    enabled: userId !== null,
  });

  const [newTodoTitle, setNewTodoTitle] = useState("");

  const createTodoMutation = useMutation({
    ...mutations.createTodo(),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.listTodos(userId!, filterTitle || null),
      });
      setNewTodoTitle("");
    },
  });

  const deleteTodoMutation = useMutation({
    ...mutations.deleteTodo(),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.listTodos(userId!, filterTitle || null),
      });
    },
  });

  return (
    <div style={{ flex: 1 }}>
      <h2>
        Todos
        {getUserQuery.data ? ` — ${getUserQuery.data.name}` : ""}
      </h2>

      {userId === null ? (
        <p>Select a user to see their todos.</p>
      ) : (
        <>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (newTodoTitle) {
                createTodoMutation.mutate({
                  title: newTodoTitle,
                  userId,
                });
              }
            }}
            style={{
              display: "flex",
              gap: "0.5rem",
            }}
          >
            <input
              placeholder="New todo"
              value={newTodoTitle}
              onChange={(e) => setNewTodoTitle(e.target.value)}
              style={{ flex: 1 }}
            />
            <button type="submit" disabled={createTodoMutation.isPending}>
              Add
            </button>
          </form>

          <input
            placeholder="Filter by title..."
            value={filterTitle}
            onChange={(e) => setFilterTitle(e.target.value)}
            style={{ width: "100%", marginTop: "0.5rem" }}
          />

          {todosQuery.isLoading && <p>Loading todos...</p>}
          {todosQuery.isError && <p>Error loading todos</p>}

          <ul style={{ listStyle: "none", padding: 0 }}>
            {todosQuery.data?.map((todo) => (
              <li
                key={todo.id}
                style={{
                  padding: "0.5rem",
                  marginBottom: "0.25rem",
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <span>{todo.title}</span>
                <button
                  onClick={() => deleteTodoMutation.mutate({ id: todo.id })}
                  disabled={deleteTodoMutation.isPending}
                >
                  x
                </button>
              </li>
            ))}
            {todosQuery.data?.length === 0 && (
              <p>{filterTitle ? "No matching todos." : "No todos yet."}</p>
            )}
          </ul>
        </>
      )}
    </div>
  );
}

function App() {
  const [selectedUser, setSelectedUser] = useState<number | null>(null);

  return (
    <main className="container">
      <h1>Welcome to Tauri + React</h1>

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

      <div style={{ display: "flex", gap: "2rem", textAlign: "left" }}>
        <UsersPanel
          selectedUser={selectedUser}
          onSelectUser={setSelectedUser}
        />
        <TodosPanel userId={selectedUser} />
      </div>
    </main>
  );
}

export default App;
