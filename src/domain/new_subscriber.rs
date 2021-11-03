use crate::domain::{SubscriberEmail, SubscriberName};
use crate::routes::SubscriberData;
use std::convert::TryFrom;

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}

impl TryFrom<SubscriberData> for NewSubscriber {
    type Error = String;

    fn try_from(data: SubscriberData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(data.name)?;
        let email = SubscriberEmail::parse(data.email)?;
        Ok(NewSubscriber { email, name })
    }
}
