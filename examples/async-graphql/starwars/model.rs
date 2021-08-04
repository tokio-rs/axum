use super::StarWars;
use async_graphql::connection::{query, Connection, Edge, EmptyFields};
use async_graphql::{Context, Enum, FieldResult, Interface, Object};

/// One of the films in the Star Wars Trilogy
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum Episode {
    /// Released in 1977.
    NewHope,

    /// Released in 1980.
    Empire,

    /// Released in 1983.
    Jedi,
}

pub struct Human(usize);

/// A humanoid creature in the Star Wars universe.
#[Object]
impl Human {
    /// The id of the human.
    async fn id(&self, ctx: &Context<'_>) -> &str {
        ctx.data_unchecked::<StarWars>().chars[self.0].id
    }

    /// The name of the human.
    async fn name(&self, ctx: &Context<'_>) -> &str {
        ctx.data_unchecked::<StarWars>().chars[self.0].name
    }

    /// The friends of the human, or an empty list if they have none.
    async fn friends(&self, ctx: &Context<'_>) -> Vec<Character> {
        ctx.data_unchecked::<StarWars>().chars[self.0]
            .friends
            .iter()
            .map(|id| Human(*id).into())
            .collect()
    }

    /// Which movies they appear in.
    async fn appears_in<'a>(&self, ctx: &'a Context<'_>) -> &'a [Episode] {
        &ctx.data_unchecked::<StarWars>().chars[self.0].appears_in
    }

    /// The home planet of the human, or null if unknown.
    async fn home_planet<'a>(&self, ctx: &'a Context<'_>) -> &'a Option<&'a str> {
        &ctx.data_unchecked::<StarWars>().chars[self.0].home_planet
    }
}

pub struct Droid(usize);

/// A mechanical creature in the Star Wars universe.
#[Object]
impl Droid {
    /// The id of the droid.
    async fn id(&self, ctx: &Context<'_>) -> &str {
        ctx.data_unchecked::<StarWars>().chars[self.0].id
    }

    /// The name of the droid.
    async fn name(&self, ctx: &Context<'_>) -> &str {
        ctx.data_unchecked::<StarWars>().chars[self.0].name
    }

    /// The friends of the droid, or an empty list if they have none.
    async fn friends(&self, ctx: &Context<'_>) -> Vec<Character> {
        ctx.data_unchecked::<StarWars>().chars[self.0]
            .friends
            .iter()
            .map(|id| Droid(*id).into())
            .collect()
    }

    /// Which movies they appear in.
    async fn appears_in<'a>(&self, ctx: &'a Context<'_>) -> &'a [Episode] {
        &ctx.data_unchecked::<StarWars>().chars[self.0].appears_in
    }

    /// The primary function of the droid.
    async fn primary_function<'a>(&self, ctx: &'a Context<'_>) -> &'a Option<&'a str> {
        &ctx.data_unchecked::<StarWars>().chars[self.0].primary_function
    }
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hero(
        &self,
        ctx: &Context<'_>,
        #[graphql(
            desc = "If omitted, returns the hero of the whole saga. If provided, returns the hero of that particular episode."
        )]
        episode: Episode,
    ) -> Character {
        if episode == Episode::Empire {
            Human(ctx.data_unchecked::<StarWars>().luke).into()
        } else {
            Droid(ctx.data_unchecked::<StarWars>().artoo).into()
        }
    }

    async fn human(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "id of the human")] id: String,
    ) -> Option<Human> {
        ctx.data_unchecked::<StarWars>().human(&id).map(Human)
    }

    async fn humans(
        &self,
        ctx: &Context<'_>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> FieldResult<Connection<usize, Human, EmptyFields, EmptyFields>> {
        let humans = ctx
            .data_unchecked::<StarWars>()
            .humans()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        query_characters(after, before, first, last, &humans)
            .await
            .map(|conn| conn.map_node(Human))
    }

    async fn droid(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "id of the droid")] id: String,
    ) -> Option<Droid> {
        ctx.data_unchecked::<StarWars>().droid(&id).map(Droid)
    }

    async fn droids(
        &self,
        ctx: &Context<'_>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> FieldResult<Connection<usize, Droid, EmptyFields, EmptyFields>> {
        let droids = ctx
            .data_unchecked::<StarWars>()
            .droids()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        query_characters(after, before, first, last, &droids)
            .await
            .map(|conn| conn.map_node(Droid))
    }
}

#[derive(Interface)]
#[graphql(
    field(name = "id", type = "&str"),
    field(name = "name", type = "&str"),
    field(name = "friends", type = "Vec<Character>"),
    field(name = "appears_in", type = "&'ctx [Episode]")
)]
pub enum Character {
    Human(Human),
    Droid(Droid),
}

async fn query_characters(
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
    characters: &[usize],
) -> FieldResult<Connection<usize, usize, EmptyFields, EmptyFields>> {
    query(
        after,
        before,
        first,
        last,
        |after, before, first, last| async move {
            let mut start = 0usize;
            let mut end = characters.len();

            if let Some(after) = after {
                if after >= characters.len() {
                    return Ok(Connection::new(false, false));
                }
                start = after + 1;
            }

            if let Some(before) = before {
                if before == 0 {
                    return Ok(Connection::new(false, false));
                }
                end = before;
            }

            let mut slice = &characters[start..end];

            if let Some(first) = first {
                slice = &slice[..first.min(slice.len())];
                end -= first.min(slice.len());
            } else if let Some(last) = last {
                slice = &slice[slice.len() - last.min(slice.len())..];
                start = end - last.min(slice.len());
            }

            let mut connection = Connection::new(start > 0, end < characters.len());
            connection.append(
                slice
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| Edge::new(start + idx, *item)),
            );
            Ok(connection)
        },
    )
    .await
}
